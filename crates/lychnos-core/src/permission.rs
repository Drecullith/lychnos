//! Permission and authorization policy.

use crate::{
    action::{ActionImpact, ActionProposal},
    runtime::{RuntimeController, RuntimeMode},
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
    /// Evaluates an action against the live Lychnos runtime safety state.
    #[must_use]
    pub fn evaluate(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
    ) -> PermissionDecision {
        Self::evaluate_mode(proposal, runtime.mode())
    }

    const fn evaluate_mode(
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
        let runtime = RuntimeController::default();

        let decision =
            DefaultPermissionPolicy.evaluate(&proposal(ActionImpact::ReadOnly), &runtime);

        assert_eq!(decision, PermissionDecision::Allowed);
    }

    #[test]
    fn normal_mode_requires_approval_for_state_changes() {
        let runtime = RuntimeController::default();

        let decision =
            DefaultPermissionPolicy.evaluate(&proposal(ActionImpact::StateChanging), &runtime);

        assert_eq!(decision, PermissionDecision::RequiresUserApproval);
    }

    #[test]
    fn game_mode_denies_actions() {
        let runtime = RuntimeController::default();
        runtime.enter_game_mode();

        let decision =
            DefaultPermissionPolicy.evaluate(&proposal(ActionImpact::ReadOnly), &runtime);

        assert_eq!(decision, PermissionDecision::Denied(DenialReason::GameMode));
    }

    #[test]
    fn disabled_mode_denies_actions() {
        let runtime = RuntimeController::default();
        runtime.disable();

        let decision =
            DefaultPermissionPolicy.evaluate(&proposal(ActionImpact::ReadOnly), &runtime);

        assert_eq!(decision, PermissionDecision::Denied(DenialReason::Disabled));
    }

    #[test]
    fn disabling_runtime_changes_subsequent_permission_decisions() {
        let runtime = RuntimeController::default();
        let proposal = proposal(ActionImpact::ReadOnly);

        assert_eq!(
            DefaultPermissionPolicy.evaluate(&proposal, &runtime),
            PermissionDecision::Allowed
        );

        runtime.disable();

        assert_eq!(
            DefaultPermissionPolicy.evaluate(&proposal, &runtime),
            PermissionDecision::Denied(DenialReason::Disabled)
        );
    }
}
