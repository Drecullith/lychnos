//! Machine-independent orchestration for the Lychnos foundation runtime.

use std::{collections::BTreeMap, sync::mpsc::TryRecvError};

use crate::{
    action::{ActionId, ActionProposal},
    analyzer::MockAnalyzer,
    approval::ApprovalGrant,
    audit::{AuditDetails, AuditEventKind, AuditRecord, AuditSink, AuditValue, InMemoryAuditLog},
    bus::{EventSubscription, InMemoryEventBus, PublishReport},
    collector::Collector,
    config::LychnosConfig,
    event::{Event, EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::{MockExecutionOutcome, MockExecutor},
    providers::{IdProvider, TimeProvider},
    runtime::{RuntimeController, RuntimeMode, RuntimeTransition},
};

/// Result of processing one event through the foundation pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationCycle {
    pub publish_report: PublishReport,
    pub event: Event,
    pub proposal: ActionProposal,
    pub outcome: MockExecutionOutcome,
    pub audit_records: usize,
}

/// Internal foundation-runtime delivery failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundationRuntimeError {
    EventQueueEmpty,
    EventSubscriptionDisconnected,
}

impl From<TryRecvError> for FoundationRuntimeError {
    fn from(error: TryRecvError) -> Self {
        match error {
            TryRecvError::Empty => Self::EventQueueEmpty,
            TryRecvError::Disconnected => Self::EventSubscriptionDisconnected,
        }
    }
}

/// Failure while collecting and processing one foundation event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectorCycleError<E> {
    /// The collector itself failed.
    Collector(E),

    /// The event reached the runtime but internal delivery failed.
    Runtime(FoundationRuntimeError),
}

/// Failure while acting on a proposal that should still be pending approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingActionError {
    /// No pending proposal exists with this action identifier.
    NotPending { action_id: ActionId },

    /// The caller is trying to approve a different proposal than the one
    /// currently held by the runtime under the same action identifier.
    ProposalChanged { action_id: ActionId },
}

/// Machine-independent orchestration service used during the foundation phase.
///
/// It deliberately uses the mock analyzer, mock executor, in-memory event bus,
/// and in-memory audit log. Those implementations can later be replaced
/// without making the CLI responsible for wiring the processing pipeline.
#[derive(Debug)]
pub struct FoundationRuntime<I, T> {
    bus: InMemoryEventBus,
    subscription: EventSubscription,
    runtime: RuntimeController,
    audit: InMemoryAuditLog,
    pending_actions: BTreeMap<ActionId, ActionProposal>,
    ids: I,
    clock: T,
}

impl<I, T> FoundationRuntime<I, T>
where
    I: IdProvider,
    T: TimeProvider,
{
    /// Creates a foundation runtime with explicit providers and startup mode.
    #[must_use]
    pub fn new(initial_mode: RuntimeMode, ids: I, clock: T) -> Self {
        let bus = InMemoryEventBus::new();
        let subscription = bus.subscribe();

        Self {
            bus,
            subscription,
            runtime: RuntimeController::new(initial_mode),
            audit: InMemoryAuditLog::new(),
            pending_actions: BTreeMap::new(),
            ids,
            clock,
        }
    }

    /// Creates a foundation runtime from validated typed configuration.
    ///
    /// Phase 2 currently consumes the configured startup mode here. Other
    /// configuration sections will be wired into their owning subsystems
    /// incrementally rather than being interpreted prematurely.
    #[must_use]
    pub fn from_config(config: &LychnosConfig, ids: I, clock: T) -> Self {
        Self::new(config.runtime.startup_mode.into(), ids, clock)
    }

    /// Returns the current runtime safety mode.
    #[must_use]
    pub fn mode(&self) -> RuntimeMode {
        self.runtime.mode()
    }

    /// Returns the append-only foundation audit log.
    #[must_use]
    pub fn audit_log(&self) -> &InMemoryAuditLog {
        &self.audit
    }

    /// Returns the pending proposal with the supplied action ID.
    #[must_use]
    pub fn pending_action(&self, action_id: &ActionId) -> Option<&ActionProposal> {
        self.pending_actions.get(action_id)
    }

    /// Returns the number of proposals currently awaiting explicit approval.
    #[must_use]
    pub fn pending_actions_len(&self) -> usize {
        self.pending_actions.len()
    }

    /// Evaluates one structured proposal through the mock execution boundary.
    ///
    /// Proposals requiring explicit approval are retained by the runtime until
    /// an exact matching proposal is approved successfully.
    #[must_use]
    pub fn evaluate_proposal(&mut self, proposal: &ActionProposal) -> MockExecutionOutcome {
        let outcome = match MockExecutor.evaluate_and_audit(
            proposal,
            &self.runtime,
            &mut self.audit,
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
        ) {
            Ok(outcome) => outcome,
            Err(never) => match never {},
        };

        if matches!(outcome, MockExecutionOutcome::AwaitingUserApproval { .. }) {
            self.pending_actions
                .insert(proposal.id.clone(), proposal.clone());
        }

        outcome
    }

    /// Approves the exact pending proposal supplied by the caller and
    /// immediately re-evaluates it against live runtime safety.
    ///
    /// Supplying an older or modified proposal with the same action ID is
    /// rejected before an approval grant is issued.
    pub fn approve_pending_action(
        &mut self,
        proposal: &ActionProposal,
        approved_by: impl Into<String>,
    ) -> Result<MockExecutionOutcome, PendingActionError> {
        let Some(pending) = self.pending_actions.get(&proposal.id) else {
            return Err(PendingActionError::NotPending {
                action_id: proposal.id.clone(),
            });
        };

        if pending != proposal {
            return Err(PendingActionError::ProposalChanged {
                action_id: proposal.id.clone(),
            });
        }

        let pending = pending.clone();
        let approval = self.issue_approval(&pending, approved_by);

        let outcome = match MockExecutor.evaluate_with_approval_and_audit(
            &pending,
            &self.runtime,
            approval,
            &mut self.audit,
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
        ) {
            Ok(outcome) => outcome,
            Err(never) => match never {},
        };

        if matches!(outcome, MockExecutionOutcome::WouldExecute { .. }) {
            self.pending_actions.remove(&pending.id);
        }

        Ok(outcome)
    }

    /// Rejects one exact pending proposal.
    ///
    /// Rejection is not gated by runtime mode: a user must always be able to
    /// refuse an action, including while Lychnos is in Game Mode or Disabled.
    ///
    /// The pending entry is removed before the audit record is appended so a
    /// future audit-storage failure cannot undo the user's rejection.
    pub fn reject_pending_action(
        &mut self,
        proposal: &ActionProposal,
        rejected_by: impl Into<String>,
    ) -> Result<(), PendingActionError> {
        let Some(pending) = self.pending_actions.get(&proposal.id) else {
            return Err(PendingActionError::NotPending {
                action_id: proposal.id.clone(),
            });
        };

        if pending != proposal {
            return Err(PendingActionError::ProposalChanged {
                action_id: proposal.id.clone(),
            });
        }

        let pending = pending.clone();
        let rejected_by = rejected_by.into();

        self.pending_actions.remove(&pending.id);

        let mut record = AuditRecord::new(
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
            AuditEventKind::ActionRejected,
            rejected_by,
            "Action proposal explicitly rejected",
        )
        .with_action(pending.id.clone())
        .with_details(
            AuditDetails::new()
                .with_field(
                    "action_kind",
                    AuditValue::Text(pending.kind.as_str().into()),
                )
                .with_field(
                    "capability",
                    AuditValue::Text(pending.capability.as_str().into()),
                )
                .with_field("impact", AuditValue::Text(format!("{:?}", pending.impact)))
                .with_field("risk", AuditValue::Text(format!("{:?}", pending.risk))),
        );

        if let Some(event_id) = &pending.source_event_id {
            record = record.with_event(event_id.clone());
        }

        match self.audit.append(record) {
            Ok(()) => {}
            Err(never) => match never {},
        }

        Ok(())
    }

    /// Issues and audits one approval grant after the pending-action boundary
    /// has verified that the user is approving the exact stored proposal.
    #[must_use]
    fn issue_approval(
        &mut self,
        proposal: &ActionProposal,
        approved_by: impl Into<String>,
    ) -> ApprovalGrant {
        let approved_by = approved_by.into();

        let mut record = AuditRecord::new(
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
            AuditEventKind::ActionApproved,
            approved_by.clone(),
            "Action proposal explicitly approved",
        )
        .with_action(proposal.id.clone())
        .with_details(
            AuditDetails::new()
                .with_field(
                    "action_kind",
                    AuditValue::Text(proposal.kind.as_str().into()),
                )
                .with_field(
                    "capability",
                    AuditValue::Text(proposal.capability.as_str().into()),
                )
                .with_field("impact", AuditValue::Text(format!("{:?}", proposal.impact)))
                .with_field("risk", AuditValue::Text(format!("{:?}", proposal.risk))),
        );

        if let Some(event_id) = &proposal.source_event_id {
            record = record.with_event(event_id.clone());
        }

        match self.audit.append(record) {
            Ok(()) => {}
            Err(never) => match never {},
        }

        ApprovalGrant::new(proposal, approved_by)
    }

    /// Enters Game Mode and records the requested runtime transition.
    pub fn enter_game_mode(&mut self) -> RuntimeTransition {
        let transition = self.runtime.enter_game_mode();
        self.audit_runtime_transition("enter_game_mode", transition);
        transition
    }

    /// Disables Lychnos and records the requested runtime transition.
    pub fn disable(&mut self) -> RuntimeTransition {
        let transition = self.runtime.disable();
        self.audit_runtime_transition("disable", transition);
        transition
    }

    /// Explicitly enables Normal mode and records the requested transition.
    pub fn enable_normal(&mut self) -> RuntimeTransition {
        let transition = self.runtime.enable_normal();
        self.audit_runtime_transition("enable_normal", transition);
        transition
    }

    fn audit_runtime_transition(&mut self, operation: &'static str, transition: RuntimeTransition) {
        let record = AuditRecord::new(
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
            AuditEventKind::RuntimeModeChanged,
            "foundation-runtime",
            "Runtime mode transition requested",
        )
        .with_details(
            AuditDetails::new()
                .with_field("operation", AuditValue::Text(operation.into()))
                .with_field("from", AuditValue::Text(format!("{:?}", transition.from)))
                .with_field("to", AuditValue::Text(format!("{:?}", transition.to)))
                .with_field("changed", AuditValue::Boolean(transition.changed)),
        );

        match self.audit.append(record) {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    /// Creates and processes one normalized foundation event.
    pub fn observe(
        &mut self,
        source: EventSource,
        kind: EventKind,
        severity: Severity,
        sensitivity: Sensitivity,
        payload: EventPayload,
    ) -> Result<FoundationCycle, FoundationRuntimeError> {
        let event = Event {
            id: self.ids.next_event_id(),
            occurred_at: self.clock.event_timestamp(),
            source,
            kind,
            severity,
            sensitivity,
            correlation_id: None,
            payload,
        };

        self.process_event(event)
    }

    /// Pulls one event from a collector and, when present, processes it
    /// through the existing foundation pipeline.
    pub fn collect_once<C: Collector>(
        &mut self,
        collector: &mut C,
    ) -> Result<Option<FoundationCycle>, CollectorCycleError<C::Error>> {
        if !self.runtime.background_work_allowed() {
            return Ok(None);
        }

        let event = collector
            .collect()
            .map_err(CollectorCycleError::Collector)?;

        let Some(event) = event else {
            return Ok(None);
        };

        self.process_event(event)
            .map(Some)
            .map_err(CollectorCycleError::Runtime)
    }

    /// Processes an already-normalized event through the foundation pipeline.
    pub fn process_event(
        &mut self,
        event: Event,
    ) -> Result<FoundationCycle, FoundationRuntimeError> {
        let publish_report = self.bus.publish(event);

        let received = self.subscription.try_recv()?;

        let proposal = MockAnalyzer.analyze(&received);

        let outcome = self.evaluate_proposal(&proposal);

        Ok(FoundationCycle {
            publish_report,
            event: received,
            proposal,
            outcome,
            audit_records: self.audit.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
        collector::MockCollector,
        event::{EventId, EventTimestamp},
        permission::DenialReason,
        providers::{FixedTimeProvider, SequenceIdProvider},
    };

    fn runtime(mode: RuntimeMode) -> FoundationRuntime<SequenceIdProvider, FixedTimeProvider> {
        FoundationRuntime::new(
            mode,
            SequenceIdProvider::default(),
            FixedTimeProvider::new(1_800_000_000_123),
        )
    }

    #[test]
    fn typed_config_controls_runtime_startup_mode() {
        let cases = [
            (crate::config::StartupMode::Normal, RuntimeMode::Normal),
            (crate::config::StartupMode::GameMode, RuntimeMode::GameMode),
            (crate::config::StartupMode::Disabled, RuntimeMode::Disabled),
        ];

        for (startup_mode, expected_mode) in cases {
            let mut config = crate::config::LychnosConfig::default();
            config.runtime.startup_mode = startup_mode;

            let runtime = FoundationRuntime::from_config(
                &config,
                SequenceIdProvider::default(),
                FixedTimeProvider::new(1_800_000_000_123),
            );

            assert_eq!(runtime.mode(), expected_mode);
        }
    }

    fn state_changing_proposal(id: &str) -> ActionProposal {
        ActionProposal::new(
            ActionId::new(id),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::Moderate,
            "Write a configuration file",
            "test-analyzer",
        )
        .with_source_event(EventId::new(format!("event-for-{id}")))
    }

    #[test]
    fn state_changing_proposal_is_held_pending() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-pending");

        let outcome = runtime.evaluate_proposal(&proposal);

        assert_eq!(
            outcome,
            MockExecutionOutcome::AwaitingUserApproval {
                action_id: ActionId::new("action-pending"),
            }
        );

        assert_eq!(runtime.pending_actions_len(), 1);
        assert_eq!(runtime.pending_action(&proposal.id), Some(&proposal));
    }

    #[test]
    fn exact_pending_approval_advances_and_clears_action() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-approved");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let outcome = runtime
            .approve_pending_action(&proposal, "local-user")
            .expect("exact pending proposal should be approvable");

        assert_eq!(
            outcome,
            MockExecutionOutcome::WouldExecute {
                action_id: ActionId::new("action-approved"),
            }
        );

        assert_eq!(runtime.pending_actions_len(), 0);
        assert!(runtime.pending_action(&proposal.id).is_none());

        let records = runtime.audit_log().records();

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].kind, AuditEventKind::PermissionEvaluated);
        assert_eq!(records[1].kind, AuditEventKind::ActionApproved);
        assert_eq!(records[1].actor, "local-user");
        assert_eq!(records[2].kind, AuditEventKind::PermissionEvaluated);

        assert_eq!(
            records[2].details.get("approval"),
            Some(&AuditValue::Text("matched".into()))
        );
        assert_eq!(
            records[2].details.get("decision"),
            Some(&AuditValue::Text("would_execute".into()))
        );
    }

    #[test]
    fn changed_pending_proposal_rejects_stale_approval() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let original = state_changing_proposal("action-changing");
        assert!(matches!(
            runtime.evaluate_proposal(&original),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let changed = ActionProposal::new(
            ActionId::new("action-changing"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::High,
            "Write different configuration contents",
            "test-analyzer",
        )
        .with_source_event(EventId::new("event-for-action-changing"));

        assert!(matches!(
            runtime.evaluate_proposal(&changed),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let error = runtime
            .approve_pending_action(&original, "local-user")
            .expect_err("stale proposal must not be approved");

        assert_eq!(
            error,
            PendingActionError::ProposalChanged {
                action_id: ActionId::new("action-changing"),
            }
        );

        assert_eq!(runtime.pending_action(&original.id), Some(&changed));

        assert!(
            runtime
                .audit_log()
                .records()
                .iter()
                .all(|record| record.kind != AuditEventKind::ActionApproved)
        );
    }

    #[test]
    fn disable_before_pending_approval_blocks_and_keeps_action_pending() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-disabled");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));
        runtime.disable();

        let outcome = runtime
            .approve_pending_action(&proposal, "local-user")
            .expect("exact proposal can be approved even though runtime safety blocks it");

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-disabled"),
                reason: DenialReason::Disabled,
            }
        );

        assert_eq!(runtime.pending_actions_len(), 1);
        assert_eq!(runtime.pending_action(&proposal.id), Some(&proposal));

        let last = runtime
            .audit_log()
            .records()
            .last()
            .expect("blocked approval evaluation should be audited");

        assert_eq!(last.kind, AuditEventKind::PermissionEvaluated);
        assert_eq!(
            last.details.get("approval"),
            Some(&AuditValue::Text("runtime_blocked".into()))
        );
        assert_eq!(
            last.details.get("decision"),
            Some(&AuditValue::Text("blocked_disabled".into()))
        );
    }

    #[test]
    fn unknown_proposal_cannot_be_approved() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-unknown");

        let error = runtime
            .approve_pending_action(&proposal, "local-user")
            .expect_err("proposal was never submitted to the runtime");

        assert_eq!(
            error,
            PendingActionError::NotPending {
                action_id: ActionId::new("action-unknown"),
            }
        );

        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn exact_pending_rejection_removes_action_and_is_audited() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-rejected");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        runtime
            .reject_pending_action(&proposal, "local-user")
            .expect("exact pending proposal should be rejectable");

        assert_eq!(runtime.pending_actions_len(), 0);
        assert!(runtime.pending_action(&proposal.id).is_none());

        let records = runtime.audit_log().records();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].kind, AuditEventKind::PermissionEvaluated);
        assert_eq!(records[1].kind, AuditEventKind::ActionRejected);
        assert_eq!(records[1].actor, "local-user");

        assert_eq!(
            records[1].action_id.as_ref().map(ActionId::as_str),
            Some("action-rejected")
        );

        assert_eq!(
            records[1].event_id.as_ref().map(EventId::as_str),
            Some("event-for-action-rejected")
        );

        assert_eq!(
            records[1].details.get("action_kind"),
            Some(&AuditValue::Text("file.write".into()))
        );
    }

    #[test]
    fn stale_proposal_cannot_reject_replacement() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let original = state_changing_proposal("action-replaced");
        assert!(matches!(
            runtime.evaluate_proposal(&original),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let replacement = ActionProposal::new(
            ActionId::new("action-replaced"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::High,
            "Replacement proposal",
            "test-analyzer",
        )
        .with_source_event(EventId::new("event-for-action-replaced"));

        assert!(matches!(
            runtime.evaluate_proposal(&replacement),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let error = runtime
            .reject_pending_action(&original, "local-user")
            .expect_err("stale proposal must not reject its replacement");

        assert_eq!(
            error,
            PendingActionError::ProposalChanged {
                action_id: ActionId::new("action-replaced"),
            }
        );

        assert_eq!(runtime.pending_action(&original.id), Some(&replacement));

        assert!(
            runtime
                .audit_log()
                .records()
                .iter()
                .all(|record| record.kind != AuditEventKind::ActionRejected)
        );
    }

    #[test]
    fn unknown_proposal_cannot_be_rejected() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-never-pending");

        let error = runtime
            .reject_pending_action(&proposal, "local-user")
            .expect_err("proposal was never pending");

        assert_eq!(
            error,
            PendingActionError::NotPending {
                action_id: ActionId::new("action-never-pending"),
            }
        );

        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn rejection_remains_available_while_disabled() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-reject-disabled");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        runtime.disable();

        runtime
            .reject_pending_action(&proposal, "local-user")
            .expect("Disabled must never prevent user rejection");

        assert!(runtime.pending_action(&proposal.id).is_none());

        let last = runtime
            .audit_log()
            .records()
            .last()
            .expect("rejection should be audited");

        assert_eq!(last.kind, AuditEventKind::ActionRejected);
        assert_eq!(last.actor, "local-user");
    }

    #[test]
    fn entering_game_mode_is_audited() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let transition = runtime.enter_game_mode();

        assert_eq!(transition.from, RuntimeMode::Normal);
        assert_eq!(transition.to, RuntimeMode::GameMode);
        assert!(transition.changed);
        assert_eq!(runtime.mode(), RuntimeMode::GameMode);

        let record = &runtime.audit_log().records()[0];

        assert_eq!(record.kind, AuditEventKind::RuntimeModeChanged);
        assert_eq!(record.actor, "foundation-runtime");
        assert_eq!(
            record.details.get("operation"),
            Some(&AuditValue::Text("enter_game_mode".into()))
        );
        assert_eq!(
            record.details.get("from"),
            Some(&AuditValue::Text("Normal".into()))
        );
        assert_eq!(
            record.details.get("to"),
            Some(&AuditValue::Text("GameMode".into()))
        );
        assert_eq!(
            record.details.get("changed"),
            Some(&AuditValue::Boolean(true))
        );
    }

    #[test]
    fn disabling_from_game_mode_records_both_transitions() {
        let mut runtime = runtime(RuntimeMode::Normal);

        runtime.enter_game_mode();
        let transition = runtime.disable();

        assert_eq!(transition.from, RuntimeMode::GameMode);
        assert_eq!(transition.to, RuntimeMode::Disabled);
        assert!(transition.changed);
        assert_eq!(runtime.mode(), RuntimeMode::Disabled);

        assert_eq!(runtime.audit_log().len(), 2);
        assert_eq!(
            runtime.audit_log().records()[1].details.get("operation"),
            Some(&AuditValue::Text("disable".into()))
        );
    }

    #[test]
    fn blocked_game_mode_request_while_disabled_is_audited() {
        let mut runtime = runtime(RuntimeMode::Disabled);

        let transition = runtime.enter_game_mode();

        assert_eq!(transition.from, RuntimeMode::Disabled);
        assert_eq!(transition.to, RuntimeMode::Disabled);
        assert!(!transition.changed);

        let record = &runtime.audit_log().records()[0];

        assert_eq!(
            record.details.get("changed"),
            Some(&AuditValue::Boolean(false))
        );
        assert_eq!(
            record.details.get("from"),
            Some(&AuditValue::Text("Disabled".into()))
        );
        assert_eq!(
            record.details.get("to"),
            Some(&AuditValue::Text("Disabled".into()))
        );
    }

    #[test]
    fn explicit_enable_from_disabled_is_audited() {
        let mut runtime = runtime(RuntimeMode::Disabled);

        let transition = runtime.enable_normal();

        assert_eq!(transition.from, RuntimeMode::Disabled);
        assert_eq!(transition.to, RuntimeMode::Normal);
        assert!(transition.changed);
        assert_eq!(runtime.mode(), RuntimeMode::Normal);

        assert_eq!(
            runtime.audit_log().records()[0].details.get("operation"),
            Some(&AuditValue::Text("enable_normal".into()))
        );
    }

    #[test]
    fn disabling_runtime_blocks_next_cycle_and_audits_both_events() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let transition = runtime.disable();

        assert_eq!(transition.from, RuntimeMode::Normal);
        assert_eq!(transition.to, RuntimeMode::Disabled);
        assert!(transition.changed);

        let cycle = runtime
            .observe(
                EventSource::new("test-source"),
                EventKind::new("test.event"),
                Severity::Info,
                Sensitivity::Standard,
                EventPayload::new(),
            )
            .expect("foundation event should process");

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-event-000002"),
                reason: DenialReason::Disabled,
            }
        );

        assert_eq!(cycle.audit_records, 2);

        let records = runtime.audit_log().records();

        assert_eq!(records[0].kind, AuditEventKind::RuntimeModeChanged);
        assert_eq!(
            records[0].details.get("operation"),
            Some(&AuditValue::Text("disable".into()))
        );

        assert_eq!(records[1].kind, AuditEventKind::PermissionEvaluated);
        assert_eq!(
            records[1].details.get("decision"),
            Some(&AuditValue::Text("blocked_disabled".into()))
        );
    }

    #[test]
    fn normal_observation_runs_complete_foundation_cycle() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let cycle = runtime
            .observe(
                EventSource::new("test-source"),
                EventKind::new("test.event"),
                Severity::Info,
                Sensitivity::Standard,
                EventPayload::new(),
            )
            .expect("foundation event should process");

        assert_eq!(cycle.event.id.as_str(), "event-000001");
        assert_eq!(cycle.event.occurred_at.as_unix_millis(), 1_800_000_000_123);
        assert_eq!(cycle.publish_report.delivered, 1);

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::WouldExecute {
                action_id: ActionId::new("action-for-event-000001"),
            }
        );

        assert_eq!(cycle.audit_records, 1);
        assert_eq!(runtime.audit_log().records()[0].id.as_str(), "audit-000002");
    }

    #[test]
    fn game_mode_blocks_orchestrated_cycle() {
        let mut runtime = runtime(RuntimeMode::GameMode);

        let cycle = runtime
            .observe(
                EventSource::new("test-source"),
                EventKind::new("test.event"),
                Severity::Info,
                Sensitivity::Standard,
                EventPayload::new(),
            )
            .expect("foundation event should process");

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-event-000001"),
                reason: DenialReason::GameMode,
            }
        );

        assert_eq!(cycle.audit_records, 1);
    }

    #[test]
    fn disabled_mode_blocks_orchestrated_cycle() {
        let mut runtime = runtime(RuntimeMode::Disabled);

        let cycle = runtime
            .observe(
                EventSource::new("test-source"),
                EventKind::new("test.event"),
                Severity::Info,
                Sensitivity::Standard,
                EventPayload::new(),
            )
            .expect("foundation event should process");

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-event-000001"),
                reason: DenialReason::Disabled,
            }
        );

        assert_eq!(cycle.audit_records, 1);
    }

    #[test]
    fn mock_collector_event_flows_through_foundation_runtime() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let event = Event {
            id: EventId::new("collector-event-001"),
            occurred_at: EventTimestamp::from_unix_millis(444),
            source: EventSource::new("mock-collector"),
            kind: EventKind::new("collector.test"),
            severity: Severity::Warning,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let mut collector = MockCollector::from_events([event]);

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("mock collection should succeed")
            .expect("collector should produce one cycle");

        assert_eq!(cycle.event.id.as_str(), "collector-event-001");
        assert_eq!(cycle.event.kind.as_str(), "collector.test");

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::WouldExecute {
                action_id: ActionId::new("action-for-collector-event-001"),
            }
        );

        assert_eq!(cycle.audit_records, 1);
        assert_eq!(runtime.audit_log().records()[0].id.as_str(), "audit-000001");
        assert!(collector.is_empty());
    }

    #[test]
    fn empty_collector_produces_no_foundation_cycle() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let mut collector = MockCollector::new();

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("empty mock collection should succeed");

        assert!(cycle.is_none());
        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn disabled_runtime_does_not_poll_collector() {
        let mut runtime = runtime(RuntimeMode::Disabled);

        let event = Event {
            id: EventId::new("collector-event-disabled"),
            occurred_at: EventTimestamp::from_unix_millis(555),
            source: EventSource::new("mock-collector"),
            kind: EventKind::new("collector.test"),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let mut collector = MockCollector::from_events([event]);

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("suppressed collection should not fail");

        assert!(cycle.is_none());
        assert_eq!(collector.pending_len(), 1);
        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn game_mode_does_not_poll_collector() {
        let mut runtime = runtime(RuntimeMode::GameMode);

        let event = Event {
            id: EventId::new("collector-event-game"),
            occurred_at: EventTimestamp::from_unix_millis(666),
            source: EventSource::new("mock-collector"),
            kind: EventKind::new("collector.test"),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let mut collector = MockCollector::from_events([event]);

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("suppressed collection should not fail");

        assert!(cycle.is_none());
        assert_eq!(collector.pending_len(), 1);
        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn collector_resumes_after_explicit_reenable() {
        let mut runtime = runtime(RuntimeMode::Disabled);

        let event = Event {
            id: EventId::new("collector-event-resumed"),
            occurred_at: EventTimestamp::from_unix_millis(777),
            source: EventSource::new("mock-collector"),
            kind: EventKind::new("collector.test"),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let mut collector = MockCollector::from_events([event]);

        assert!(
            runtime
                .collect_once(&mut collector)
                .expect("suppressed collection should not fail")
                .is_none()
        );

        runtime.enable_normal();

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("collection should succeed after re-enable")
            .expect("queued event should now be processed");

        assert_eq!(cycle.event.id.as_str(), "collector-event-resumed");
        assert!(collector.is_empty());
    }

    #[test]
    fn collector_errors_are_preserved() {
        struct FailingCollector;

        impl Collector for FailingCollector {
            type Error = &'static str;

            fn collect(&mut self) -> Result<Option<Event>, Self::Error> {
                Err("collector failed")
            }
        }

        let mut runtime = runtime(RuntimeMode::Normal);
        let mut collector = FailingCollector;

        let error = runtime
            .collect_once(&mut collector)
            .expect_err("collector failure should propagate");

        assert_eq!(error, CollectorCycleError::Collector("collector failed"));

        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn already_normalized_events_can_enter_the_runtime() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let event = Event {
            id: EventId::new("collector-event-001"),
            occurred_at: EventTimestamp::from_unix_millis(123),
            source: EventSource::new("future-collector"),
            kind: EventKind::new("collector.event"),
            severity: Severity::Warning,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let cycle = runtime
            .process_event(event)
            .expect("normalized event should process");

        assert_eq!(cycle.event.id.as_str(), "collector-event-001");
        assert_eq!(cycle.event.occurred_at.as_unix_millis(), 123);

        // No event ID was generated by the runtime in this path, so the first
        // provider-generated identifier is the audit record.
        assert_eq!(runtime.audit_log().records()[0].id.as_str(), "audit-000001");
    }
}
