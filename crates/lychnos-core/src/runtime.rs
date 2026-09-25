//! Runtime safety state for Lychnos.

use std::sync::atomic::{AtomicU8, Ordering};

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

/// Thread-safe controller for Lychnos runtime safety state.
#[derive(Debug)]
pub struct RuntimeController {
    mode: AtomicU8,
}

impl Default for RuntimeController {
    fn default() -> Self {
        Self::new(RuntimeMode::Normal)
    }
}

impl RuntimeController {
    /// Creates a runtime controller with the requested initial mode.
    #[must_use]
    pub const fn new(initial_mode: RuntimeMode) -> Self {
        Self {
            mode: AtomicU8::new(initial_mode as u8),
        }
    }

    /// Returns the current runtime mode.
    #[must_use]
    pub fn mode(&self) -> RuntimeMode {
        RuntimeMode::from_stored(self.mode.load(Ordering::Acquire))
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

    /// Enters Game Mode unless Lychnos is already disabled.
    ///
    /// Disabled mode fails closed and cannot be weakened by this operation.
    pub fn enter_game_mode(&self) -> RuntimeTransition {
        loop {
            let current = self.mode();

            match current {
                RuntimeMode::Disabled => {
                    return RuntimeTransition::new(RuntimeMode::Disabled, RuntimeMode::Disabled);
                }

                RuntimeMode::GameMode => {
                    return RuntimeTransition::new(RuntimeMode::GameMode, RuntimeMode::GameMode);
                }

                RuntimeMode::Normal => {
                    if self
                        .mode
                        .compare_exchange(
                            RuntimeMode::Normal as u8,
                            RuntimeMode::GameMode as u8,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return RuntimeTransition::new(RuntimeMode::Normal, RuntimeMode::GameMode);
                    }
                }
            }
        }
    }

    /// Immediately places Lychnos into Disabled mode.
    pub fn disable(&self) -> RuntimeTransition {
        let previous = RuntimeMode::from_stored(
            self.mode
                .swap(RuntimeMode::Disabled as u8, Ordering::AcqRel),
        );

        RuntimeTransition::new(previous, RuntimeMode::Disabled)
    }

    /// Explicitly enables normal Lychnos operation.
    ///
    /// This is intentionally separate from ordinary mode changes so Disabled
    /// cannot be left accidentally.
    pub fn enable_normal(&self) -> RuntimeTransition {
        let previous =
            RuntimeMode::from_stored(self.mode.swap(RuntimeMode::Normal as u8, Ordering::AcqRel));

        RuntimeTransition::new(previous, RuntimeMode::Normal)
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
}
