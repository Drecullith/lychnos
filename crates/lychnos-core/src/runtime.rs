//! Runtime safety state for Lychnos.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Current operating mode of the Lychnos runtime.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeMode {
    /// Normal Lychnos operation.
    #[default]
    Normal = 0,

    /// Reduced-impact operation for gaming and streaming workloads.
    GameMode = 1,

    /// Lychnos is disabled and must not perform actions or background work.
    Disabled = 2,
}

impl RuntimeMode {
    /// Returns whether Lychnos may perform actions in this mode.
    #[must_use]
    pub const fn actions_allowed(self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Returns whether non-essential background work should run.
    #[must_use]
    pub const fn background_work_allowed(self) -> bool {
        matches!(self, Self::Normal)
    }

    fn from_stored(value: u8) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::GameMode,
            2 => Self::Disabled,

            // Fail closed if memory corruption or a future incompatible value
            // is ever observed.
            _ => Self::Disabled,
        }
    }
}

/// Result of one requested runtime-mode transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTransition {
    pub from: RuntimeMode,
    pub to: RuntimeMode,
    pub changed: bool,
}

impl RuntimeTransition {
    const fn new(from: RuntimeMode, to: RuntimeMode) -> Self {
        Self {
            from,
            to,
            changed: from as u8 != to as u8,
        }
    }
}

/// Work-start failure caused by the current runtime mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorkStartError {
    GameMode,
    Disabled,
}

/// Runtime request visible to work that has already started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWorkRequest {
    /// Work may continue normally.
    Continue,
    /// Game Mode requests that work cooperatively pause or quiesce.
    PauseForGameMode,
    /// Disabled requests permanent cooperative cancellation.
    CancelForDisabled,
}

/// Runtime lease held by work that has already started.
///
/// Game Mode produces a reversible pause request. Returning to Normal clears
/// that request for leases that have not crossed a Disabled transition.
///
/// A transition into Disabled permanently invalidates leases that were issued
/// before that transition. Returning to Normal does not revive old work.
#[derive(Debug, Clone)]
pub struct RuntimeWorkLease {
    state: Arc<AtomicU64>,
    disable_epoch: u64,
}

impl RuntimeWorkLease {
    /// Returns the current runtime request for this already-started work.
    ///
    /// Disabled cancellation takes precedence permanently. A Game Mode pause
    /// request is reversible and only applies while the lease still belongs to
    /// the current disable generation.
    #[must_use]
    pub fn request(&self) -> RuntimeWorkRequest {
        let state = self.state.load(Ordering::Acquire);

        if mode_from_state(state) == RuntimeMode::Disabled
            || disable_epoch_from_state(state) != self.disable_epoch
        {
            RuntimeWorkRequest::CancelForDisabled
        } else if mode_from_state(state) == RuntimeMode::GameMode {
            RuntimeWorkRequest::PauseForGameMode
        } else {
            RuntimeWorkRequest::Continue
        }
    }

    /// Returns whether Disabled has permanently requested cancellation.
    #[must_use]
    pub fn cancellation_requested(&self) -> bool {
        self.request() == RuntimeWorkRequest::CancelForDisabled
    }
}

const MODE_MASK: u64 = u8::MAX as u64;
const DISABLE_EPOCH_SHIFT: u32 = 8;
const DISABLE_EPOCH_MASK: u64 = u64::MAX >> DISABLE_EPOCH_SHIFT;

fn encode_runtime_state(mode: RuntimeMode, disable_epoch: u64) -> u64 {
    ((disable_epoch & DISABLE_EPOCH_MASK) << DISABLE_EPOCH_SHIFT) | mode as u64
}

fn mode_from_state(state: u64) -> RuntimeMode {
    RuntimeMode::from_stored((state & MODE_MASK) as u8)
}

fn disable_epoch_from_state(state: u64) -> u64 {
    state >> DISABLE_EPOCH_SHIFT
}

/// Thread-safe controller for Lychnos runtime safety state.
#[derive(Debug)]
pub struct RuntimeController {
    state: Arc<AtomicU64>,
}

impl Default for RuntimeController {
    fn default() -> Self {
        Self::new(RuntimeMode::Normal)
    }
}

impl RuntimeController {
    /// Creates a runtime controller with the requested initial mode.
    #[must_use]
    pub fn new(initial_mode: RuntimeMode) -> Self {
        Self {
            state: Arc::new(AtomicU64::new(encode_runtime_state(initial_mode, 0))),
        }
    }

    /// Returns the current runtime mode.
    #[must_use]
    pub fn mode(&self) -> RuntimeMode {
        mode_from_state(self.state.load(Ordering::Acquire))
    }

    /// Returns whether actions are currently allowed.
    #[must_use]
    pub fn actions_allowed(&self) -> bool {
        self.mode().actions_allowed()
    }

    /// Returns whether non-essential background work is currently allowed.
    #[must_use]
    pub fn background_work_allowed(&self) -> bool {
        self.mode().background_work_allowed()
    }

    /// Issues a cancellation lease for work beginning in Normal mode.
    ///
    /// One atomic runtime snapshot binds the lease to the current disable
    /// generation. A later Disabled transition invalidates that lease even if
    /// the runtime subsequently returns to Normal.
    pub fn try_begin_work(&self) -> Result<RuntimeWorkLease, RuntimeWorkStartError> {
        let state = self.state.load(Ordering::Acquire);

        match mode_from_state(state) {
            RuntimeMode::Normal => Ok(RuntimeWorkLease {
                state: Arc::clone(&self.state),
                disable_epoch: disable_epoch_from_state(state),
            }),
            RuntimeMode::GameMode => Err(RuntimeWorkStartError::GameMode),
            RuntimeMode::Disabled => Err(RuntimeWorkStartError::Disabled),
        }
    }

    /// Enters Game Mode unless Lychnos is already disabled.
    ///
    /// Disabled mode fails closed and cannot be weakened by this operation.
    pub fn enter_game_mode(&self) -> RuntimeTransition {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let current = mode_from_state(state);

            match current {
                RuntimeMode::Disabled => {
                    return RuntimeTransition::new(RuntimeMode::Disabled, RuntimeMode::Disabled);
                }

                RuntimeMode::GameMode => {
                    return RuntimeTransition::new(RuntimeMode::GameMode, RuntimeMode::GameMode);
                }

                RuntimeMode::Normal => {
                    let next = encode_runtime_state(
                        RuntimeMode::GameMode,
                        disable_epoch_from_state(state),
                    );

                    if self
                        .state
                        .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return RuntimeTransition::new(RuntimeMode::Normal, RuntimeMode::GameMode);
                    }
                }
            }
        }
    }

    /// Immediately places Lychnos into Disabled mode.
    ///
    /// A successful transition into Disabled also advances the disable epoch,
    /// permanently requesting cancellation from work that started earlier.
    pub fn disable(&self) -> RuntimeTransition {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let current = mode_from_state(state);

            if current == RuntimeMode::Disabled {
                return RuntimeTransition::new(RuntimeMode::Disabled, RuntimeMode::Disabled);
            }

            let next_epoch = disable_epoch_from_state(state).wrapping_add(1) & DISABLE_EPOCH_MASK;

            let next = encode_runtime_state(RuntimeMode::Disabled, next_epoch);

            if self
                .state
                .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return RuntimeTransition::new(current, RuntimeMode::Disabled);
            }
        }
    }

    /// Explicitly enables normal Lychnos operation.
    ///
    /// Re-enabling preserves the disable epoch, so work cancelled by an
    /// earlier Disabled transition does not silently become live again.
    pub fn enable_normal(&self) -> RuntimeTransition {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let current = mode_from_state(state);

            if current == RuntimeMode::Normal {
                return RuntimeTransition::new(RuntimeMode::Normal, RuntimeMode::Normal);
            }

            let next = encode_runtime_state(RuntimeMode::Normal, disable_epoch_from_state(state));

            if self
                .state
                .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return RuntimeTransition::new(current, RuntimeMode::Normal);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mode_allows_actions_and_background_work() {
        let mode = RuntimeMode::Normal;

        assert!(mode.actions_allowed());
        assert!(mode.background_work_allowed());
    }

    #[test]
    fn game_mode_blocks_actions_and_background_work() {
        let mode = RuntimeMode::GameMode;

        assert!(!mode.actions_allowed());
        assert!(!mode.background_work_allowed());
    }

    #[test]
    fn disabled_mode_blocks_actions_and_background_work() {
        let mode = RuntimeMode::Disabled;

        assert!(!mode.actions_allowed());
        assert!(!mode.background_work_allowed());
    }

    #[test]
    fn controller_starts_in_normal_mode_by_default() {
        let controller = RuntimeController::default();

        assert_eq!(controller.mode(), RuntimeMode::Normal);
        assert!(controller.actions_allowed());
        assert!(controller.background_work_allowed());
    }

    #[test]
    fn entering_game_mode_blocks_work() {
        let controller = RuntimeController::default();

        let transition = controller.enter_game_mode();

        assert_eq!(
            transition,
            RuntimeTransition {
                from: RuntimeMode::Normal,
                to: RuntimeMode::GameMode,
                changed: true,
            }
        );

        assert_eq!(controller.mode(), RuntimeMode::GameMode);
        assert!(!controller.actions_allowed());
        assert!(!controller.background_work_allowed());
    }

    #[test]
    fn disable_overrides_game_mode() {
        let controller = RuntimeController::default();

        controller.enter_game_mode();
        let transition = controller.disable();

        assert_eq!(transition.from, RuntimeMode::GameMode);
        assert_eq!(transition.to, RuntimeMode::Disabled);
        assert!(transition.changed);
        assert_eq!(controller.mode(), RuntimeMode::Disabled);
    }

    #[test]
    fn game_mode_cannot_reenable_disabled_runtime() {
        let controller = RuntimeController::default();

        controller.disable();
        let transition = controller.enter_game_mode();

        assert_eq!(
            transition,
            RuntimeTransition {
                from: RuntimeMode::Disabled,
                to: RuntimeMode::Disabled,
                changed: false,
            }
        );

        assert_eq!(controller.mode(), RuntimeMode::Disabled);
    }

    #[test]
    fn leaving_disabled_requires_explicit_enable() {
        let controller = RuntimeController::default();

        controller.disable();

        let transition = controller.enable_normal();

        assert_eq!(
            transition,
            RuntimeTransition {
                from: RuntimeMode::Disabled,
                to: RuntimeMode::Normal,
                changed: true,
            }
        );

        assert_eq!(controller.mode(), RuntimeMode::Normal);
    }

    #[test]
    fn work_can_begin_only_in_normal_mode() {
        let normal = RuntimeController::new(RuntimeMode::Normal);
        assert!(normal.try_begin_work().is_ok());

        let game = RuntimeController::new(RuntimeMode::GameMode);
        assert_eq!(
            game.try_begin_work()
                .expect_err("Game Mode must block new work"),
            RuntimeWorkStartError::GameMode
        );

        let disabled = RuntimeController::new(RuntimeMode::Disabled);
        assert_eq!(
            disabled
                .try_begin_work()
                .expect_err("Disabled must block new work"),
            RuntimeWorkStartError::Disabled
        );
    }

    #[test]
    fn game_mode_requests_reversible_pause_for_existing_work() {
        let controller = RuntimeController::default();
        let lease = controller
            .try_begin_work()
            .expect("Normal mode should issue work lease");

        assert_eq!(lease.request(), RuntimeWorkRequest::Continue);

        controller.enter_game_mode();

        assert_eq!(lease.request(), RuntimeWorkRequest::PauseForGameMode);

        controller.enable_normal();

        assert_eq!(lease.request(), RuntimeWorkRequest::Continue);
    }

    #[test]
    fn disabling_requests_cancellation_for_existing_work() {
        let controller = RuntimeController::default();
        let lease = controller
            .try_begin_work()
            .expect("Normal mode should issue work lease");

        assert_eq!(lease.request(), RuntimeWorkRequest::Continue);

        controller.disable();

        assert_eq!(lease.request(), RuntimeWorkRequest::CancelForDisabled);
        assert!(lease.cancellation_requested());
    }

    #[test]
    fn reenable_does_not_revive_cancelled_work() {
        let controller = RuntimeController::default();
        let lease = controller
            .try_begin_work()
            .expect("Normal mode should issue work lease");

        controller.disable();
        controller.enable_normal();

        assert_eq!(controller.mode(), RuntimeMode::Normal);
        assert!(lease.cancellation_requested());
    }

    #[test]
    fn work_started_after_reenable_gets_fresh_lease() {
        let controller = RuntimeController::default();

        let old = controller
            .try_begin_work()
            .expect("first work lease should start");

        controller.disable();
        controller.enable_normal();

        let fresh = controller
            .try_begin_work()
            .expect("new work may start after explicit re-enable");

        assert!(old.cancellation_requested());
        assert!(!fresh.cancellation_requested());
    }

    #[test]
    fn disabled_cancellation_outranks_later_game_mode_for_old_work() {
        let controller = RuntimeController::default();
        let lease = controller
            .try_begin_work()
            .expect("Normal mode should issue work lease");

        controller.disable();
        controller.enable_normal();
        controller.enter_game_mode();

        assert_eq!(lease.request(), RuntimeWorkRequest::CancelForDisabled);
    }

    #[test]
    fn disabling_from_game_mode_upgrades_pause_to_cancellation() {
        let controller = RuntimeController::default();
        let lease = controller
            .try_begin_work()
            .expect("Normal mode should issue work lease");

        controller.enter_game_mode();
        assert_eq!(lease.request(), RuntimeWorkRequest::PauseForGameMode);

        controller.disable();

        assert_eq!(lease.request(), RuntimeWorkRequest::CancelForDisabled);
    }
}
