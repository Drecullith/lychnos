//! Safe mock execution boundary for foundation-phase Lychnos.

use crate::{
    action::{ActionId, ActionProposal},
    permission::{DefaultPermissionPolicy, DenialReason, PermissionDecision},
    runtime::RuntimeController,
};

/// Result of passing an action through the mock execution boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockExecutionOutcome {
    /// The action passed policy and would be executable in a future real executor.
    WouldExecute { action_id: ActionId },

    /// The action must wait for explicit user approval.
    AwaitingUserApproval { action_id: ActionId },

    /// Runtime safety policy blocked the action.
    Blocked {
        action_id: ActionId,
        reason: DenialReason,
    },
}

/// Foundation-phase executor that never performs real system actions.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockExecutor;

impl MockExecutor {
    /// Evaluates an action against live policy without executing anything.
    #[must_use]
    pub fn evaluate(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
    ) -> MockExecutionOutcome {
        match DefaultPermissionPolicy.evaluate(proposal, runtime) {
            PermissionDecision::Allowed => MockExecutionOutcome::WouldExecute {
                action_id: proposal.id.clone(),
            },

            PermissionDecision::RequiresUserApproval => {
                MockExecutionOutcome::AwaitingUserApproval {
                    action_id: proposal.id.clone(),
                }
            }

            PermissionDecision::Denied(reason) => MockExecutionOutcome::Blocked {
                action_id: proposal.id.clone(),
                reason,
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

    fn proposal(id: &str, impact: ActionImpact) -> ActionProposal {
        ActionProposal::new(
            ActionId::new(id),
            ActionKind::new("mock.action"),
            Capability::new("mock.capability"),
            impact,
            ActionRisk::Low,
            "Foundation-phase mock action",
            "test",
        )
    }

    #[test]
    fn read_only_action_would_execute_in_normal_mode() {
        let runtime = RuntimeController::default();
        let proposal = proposal("action-001", ActionImpact::ReadOnly);

        let outcome = MockExecutor.evaluate(&proposal, &runtime);

        assert_eq!(
            outcome,
            MockExecutionOutcome::WouldExecute {
                action_id: ActionId::new("action-001"),
            }
        );
    }

    #[test]
    fn state_changing_action_waits_for_user_approval() {
        let runtime = RuntimeController::default();
        let proposal = proposal("action-002", ActionImpact::StateChanging);

        let outcome = MockExecutor.evaluate(&proposal, &runtime);

        assert_eq!(
            outcome,
            MockExecutionOutcome::AwaitingUserApproval {
                action_id: ActionId::new("action-002"),
            }
        );
    }

    #[test]
    fn game_mode_blocks_even_read_only_actions() {
        let runtime = RuntimeController::default();
        runtime.enter_game_mode();

        let proposal = proposal("action-003", ActionImpact::ReadOnly);

        let outcome = MockExecutor.evaluate(&proposal, &runtime);

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-003"),
                reason: DenialReason::GameMode,
            }
        );
    }

    #[test]
    fn disabled_mode_blocks_even_read_only_actions() {
        let runtime = RuntimeController::default();
        runtime.disable();

        let proposal = proposal("action-004", ActionImpact::ReadOnly);

        let outcome = MockExecutor.evaluate(&proposal, &runtime);

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-004"),
                reason: DenialReason::Disabled,
            }
        );
    }
}
