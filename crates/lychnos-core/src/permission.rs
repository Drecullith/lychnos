//! Permission and authorization policy.

use crate::{
    action::{ActionImpact, ActionProposal},
    runtime::RuntimeMode,
};

/// Reason an action was denied before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenialReason {
    /// Game Mode blocks actions to minimize interference.
    GameMode,

    /// Lychnos is disabled.
    Disabled,
}

/// Result of evaluating an action proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    /// The action may proceed without additional user approval.
    Allowed,

    /// The action requires explicit user approval before execution.
    RequiresUserApproval,

    /// The action is blocked.
    Denied(DenialReason),
}

/// Initial permission policy for machine-independent Lychnos foundations.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPermissionPolicy;

impl DefaultPermissionPolicy {
    /// Evaluates an action against the current Lychnos runtime mode.
    #[must_use]
    pub const fn evaluate(
        self,
        proposal: &ActionProposal,
        runtime_mode: RuntimeMode,
    ) -> PermissionDecision {
        match runtime_mode {
            RuntimeMode::Disabled => PermissionDecision::Denied(DenialReason::Disabled),

            RuntimeMode::GameMode => PermissionDecision::Denied(DenialReason::GameMode),

            RuntimeMode::Normal => match proposal.impact {
                ActionImpact::ReadOnly => PermissionDecision::Allowed,

                ActionImpact::StateChanging
                | ActionImpact::External
                | ActionImpact::Privileged
                | ActionImpact::Destructive => PermissionDecision::RequiresUserApproval,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{
        ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability,
    };

    fn proposal(impact: ActionImpact) -> ActionProposal {
        ActionProposal::new(
            ActionId::new("action-001"),
            ActionKind::new("mock.action"),
            Capability::new("mock.capability"),
            impact,
            ActionRisk::Low,
            "Test proposal",
            "test",
        )
    }

    #[test]
    fn normal_mode_allows_read_only_actions() {
        let decision = DefaultPermissionPolicy
            .evaluate(&proposal(ActionImpact::ReadOnly), RuntimeMode::Normal);

        assert_eq!(decision, PermissionDecision::Allowed);
    }

    #[test]
    fn normal_mode_requires_approval_for_state_changes() {
        let decision = DefaultPermissionPolicy
            .evaluate(&proposal(ActionImpact::StateChanging), RuntimeMode::Normal);

        assert_eq!(decision, PermissionDecision::RequiresUserApproval);
    }

    #[test]
    fn game_mode_denies_actions() {
        let decision = DefaultPermissionPolicy
            .evaluate(&proposal(ActionImpact::ReadOnly), RuntimeMode::GameMode);

        assert_eq!(decision, PermissionDecision::Denied(DenialReason::GameMode));
    }

    #[test]
    fn disabled_mode_denies_actions() {
        let decision = DefaultPermissionPolicy
            .evaluate(&proposal(ActionImpact::ReadOnly), RuntimeMode::Disabled);

        assert_eq!(decision, PermissionDecision::Denied(DenialReason::Disabled));
    }
}
