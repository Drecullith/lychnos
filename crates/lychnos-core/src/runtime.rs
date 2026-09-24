//! Runtime safety state for Lychnos.

/// Current operating mode of the Lychnos runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeMode {
    /// Normal Lychnos operation.
    #[default]
    Normal,

    /// Reduced-impact operation for gaming and streaming workloads.
    GameMode,

    /// Lychnos is disabled and must not perform actions or background work.
    Disabled,
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
}

#[cfg(test)]
mod tests {
    use super::RuntimeMode;

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
}
