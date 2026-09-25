//! Safe mock execution boundary for foundation-phase Lychnos.

use crate::{
    action::{ActionId, ActionProposal},
    audit::{
        AuditDetails, AuditEventKind, AuditId, AuditRecord, AuditSink, AuditTimestamp, AuditValue,
    },
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

impl MockExecutionOutcome {
    fn audit_label(&self) -> &'static str {
        match self {
            Self::WouldExecute { .. } => "would_execute",
            Self::AwaitingUserApproval { .. } => "awaiting_user_approval",
            Self::Blocked {
                reason: DenialReason::GameMode,
                ..
            } => "blocked_game_mode",
            Self::Blocked {
                reason: DenialReason::Disabled,
                ..
            } => "blocked_disabled",
        }
    }

    fn audit_message(&self) -> &'static str {
        match self {
            Self::WouldExecute { .. } => "Action passed mock execution policy",
            Self::AwaitingUserApproval { .. } => "Action requires explicit user approval",
            Self::Blocked { .. } => "Action blocked by runtime safety policy",
        }
    }
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

    /// Evaluates an action and appends the resulting security audit record.
    pub fn evaluate_and_audit<S: AuditSink>(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
        audit: &mut S,
        audit_id: AuditId,
        occurred_at: AuditTimestamp,
    ) -> Result<MockExecutionOutcome, S::Error> {
        let outcome = self.evaluate(proposal, runtime);

        let mut record = AuditRecord::new(
            audit_id,
            occurred_at,
            AuditEventKind::PermissionEvaluated,
            "mock-executor",
            outcome.audit_message(),
        )
        .with_action(proposal.id.clone())
        .with_details(
            AuditDetails::new()
                .with_field("decision", AuditValue::Text(outcome.audit_label().into())),
        );

        if let Some(event_id) = &proposal.source_event_id {
            record = record.with_event(event_id.clone());
        }

        audit.append(record)?;

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
        audit::InMemoryAuditLog,
        event::EventId,
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

    #[test]
    fn audited_allowed_action_records_permission_result() {
        let runtime = RuntimeController::default();
        let mut audit = InMemoryAuditLog::new();

        let proposal = proposal("action-005", ActionImpact::ReadOnly)
            .with_source_event(EventId::new("event-005"));

        let outcome = MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-005"),
                AuditTimestamp::from_unix_millis(5),
            )
            .expect("in-memory audit append cannot fail");

        assert!(matches!(outcome, MockExecutionOutcome::WouldExecute { .. }));

        assert_eq!(audit.len(), 1);

        let record = &audit.records()[0];

        assert_eq!(record.id.as_str(), "audit-005");
        assert_eq!(
            record.action_id.as_ref().map(ActionId::as_str),
            Some("action-005")
        );
        assert_eq!(
            record.event_id.as_ref().map(EventId::as_str),
            Some("event-005")
        );
        assert_eq!(
            record.details.get("decision"),
            Some(&AuditValue::Text("would_execute".into()))
        );
    }

    #[test]
    fn audited_approval_requirement_is_recorded() {
        let runtime = RuntimeController::default();
        let mut audit = InMemoryAuditLog::new();

        let proposal = proposal("action-006", ActionImpact::StateChanging);

        MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-006"),
                AuditTimestamp::from_unix_millis(6),
            )
            .expect("in-memory audit append cannot fail");

        assert_eq!(
            audit.records()[0].details.get("decision"),
            Some(&AuditValue::Text("awaiting_user_approval".into()))
        );
    }

    #[test]
    fn audited_runtime_block_is_recorded() {
        let runtime = RuntimeController::default();
        runtime.disable();

        let mut audit = InMemoryAuditLog::new();
        let proposal = proposal("action-007", ActionImpact::ReadOnly);

        MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-007"),
                AuditTimestamp::from_unix_millis(7),
            )
            .expect("in-memory audit append cannot fail");

        assert_eq!(
            audit.records()[0].details.get("decision"),
            Some(&AuditValue::Text("blocked_disabled".into()))
        );
    }
}
