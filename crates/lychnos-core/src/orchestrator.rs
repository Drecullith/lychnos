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
    diagnostics::{DiagnosticLevel, DiagnosticRecord, DiagnosticSink, InMemoryDiagnosticLog},
    event::{Event, EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::{
        MockExecutionOutcome, MockExecutor, MockRunningWork, MockRunningWorkState,
        MockRunningWorkTransitionError, MockWorkCooperationAssessment, MockWorkCooperationRequest,
    },
    providers::{IdProvider, TimeProvider},
    runtime::{RuntimeController, RuntimeMode, RuntimeTransition, RuntimeWorkStartError},
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

/// Failure while tracking simulation-only running work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockWorkError {
    /// The simulation already tracks work under this action identifier.
    AlreadyTracked { action_id: ActionId },

    /// No simulated work is tracked under this action identifier.
    NotTracked { action_id: ActionId },

    /// Live runtime safety blocked the simulated work start.
    StartBlocked(RuntimeWorkStartError),

    /// The requested simulated lifecycle transition was invalid.
    Transition(MockRunningWorkTransitionError),
}

/// Point-in-time report for one simulation-only running-work item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockWorkLifecycleSnapshot {
    pub action_id: ActionId,
    pub state: MockRunningWorkState,
    pub runtime_mode: RuntimeMode,
    pub terminal: bool,
    pub cooperation_pending: bool,
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

/// Why the runtime cancelled a pending action without treating the event as
/// explicit user rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingActionCancellationReason {
    /// A newer, structurally different proposal reused the same action ID.
    Superseded,

    /// Runtime or policy state determined that the pending request is no
    /// longer valid.
    PolicyInvalidated,

    /// A runtime lifecycle operation requires the pending request to end.
    RuntimeLifecycle,
}

impl PendingActionCancellationReason {
    /// Stable audit representation of the cancellation reason.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Superseded => "superseded",
            Self::PolicyInvalidated => "policy_invalidated",
            Self::RuntimeLifecycle => "runtime_lifecycle",
        }
    }
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
    diagnostics: InMemoryDiagnosticLog,
    diagnostics_enabled: bool,
    pending_actions: BTreeMap<ActionId, ActionProposal>,
    mock_work: BTreeMap<ActionId, MockRunningWork>,
    ids: I,
    clock: T,
}

impl<I, T> FoundationRuntime<I, T>
where
    I: IdProvider,
    T: TimeProvider,
{
    /// Creates a foundation runtime with explicit providers and startup mode.
    ///
    /// Ordinary diagnostics use their default enabled state. Security auditing
    /// remains mandatory and independent of this setting.
    #[must_use]
    pub fn new(initial_mode: RuntimeMode, ids: I, clock: T) -> Self {
        Self::new_with_diagnostics(initial_mode, true, ids, clock)
    }

    fn new_with_diagnostics(
        initial_mode: RuntimeMode,
        diagnostics_enabled: bool,
        ids: I,
        clock: T,
    ) -> Self {
        let bus = InMemoryEventBus::new();
        let subscription = bus.subscribe();

        let mut runtime = Self {
            bus,
            subscription,
            runtime: RuntimeController::new(initial_mode),
            audit: InMemoryAuditLog::new(),
            diagnostics: InMemoryDiagnosticLog::new(),
            diagnostics_enabled,
            pending_actions: BTreeMap::new(),
            mock_work: BTreeMap::new(),
            ids,
            clock,
        };

        runtime.emit_diagnostic(
            DiagnosticLevel::Info,
            "runtime",
            format!("Foundation runtime started in {initial_mode:?} mode"),
        );

        runtime
    }

    /// Creates a foundation runtime from validated typed configuration.
    ///
    /// Phase 2 currently consumes the configured startup mode here. Other
    /// configuration sections will be wired into their owning subsystems
    /// incrementally rather than being interpreted prematurely.
    #[must_use]
    pub fn from_config(config: &LychnosConfig, ids: I, clock: T) -> Self {
        Self::new_with_diagnostics(
            config.runtime.startup_mode.into(),
            config.diagnostics.enabled,
            ids,
            clock,
        )
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

    /// Returns ordinary runtime diagnostics.
    ///
    /// These records are intentionally separate from the mandatory security
    /// audit trail.
    #[must_use]
    pub fn diagnostics_log(&self) -> &InMemoryDiagnosticLog {
        &self.diagnostics
    }

    /// Returns whether ordinary diagnostics are enabled.
    #[must_use]
    pub const fn diagnostics_enabled(&self) -> bool {
        self.diagnostics_enabled
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

    /// Starts one simulation-only running-work record.
    ///
    /// This does not authorize or perform an operating-system action. It only
    /// lets Phase 2 scenarios exercise lifecycle behavior against the live
    /// runtime controller.
    pub fn start_mock_work(
        &mut self,
        action_id: ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        if self.mock_work.contains_key(&action_id) {
            return Err(MockWorkError::AlreadyTracked { action_id });
        }

        let lease = self
            .runtime
            .try_begin_work()
            .map_err(MockWorkError::StartBlocked)?;

        let work = MockRunningWork::new(action_id.clone(), lease);
        let state = work.state();
        self.mock_work.insert(action_id.clone(), work);
        self.audit_mock_work_transition(&action_id, "start", None, state);

        Ok(state)
    }

    /// Returns the current state of one tracked simulation-only work item.
    #[must_use]
    pub fn mock_work_state(&self, action_id: &ActionId) -> Option<MockRunningWorkState> {
        self.mock_work.get(action_id).map(MockRunningWork::state)
    }

    /// Returns a structured point-in-time lifecycle report for mock work.
    #[must_use]
    pub fn mock_work_snapshot(&self, action_id: &ActionId) -> Option<MockWorkLifecycleSnapshot> {
        let state = self.mock_work_state(action_id)?;

        Some(MockWorkLifecycleSnapshot {
            action_id: action_id.clone(),
            state,
            runtime_mode: self.runtime.mode(),
            terminal: state.is_terminal(),
            cooperation_pending: state.cooperation_pending(),
        })
    }

    /// Assesses whether one runtime cooperation request is still waiting,
    /// timed out, satisfied, unavailable, or no longer applicable.
    ///
    /// Elapsed time is supplied by the caller so this machine-independent
    /// boundary does not choose a timer or async runtime.
    pub fn assess_mock_work_cooperation(
        &self,
        action_id: &ActionId,
        request: MockWorkCooperationRequest,
        elapsed_ms: u64,
        timeout_ms: u64,
    ) -> Result<MockWorkCooperationAssessment, MockWorkError> {
        let state = self
            .mock_work_state(action_id)
            .ok_or_else(|| MockWorkError::NotTracked {
                action_id: action_id.clone(),
            })?;

        Ok(MockWorkCooperationAssessment::evaluate(
            request, state, elapsed_ms, timeout_ms,
        ))
    }

    /// Returns the number of simulation-only work items retained for lifecycle
    /// inspection, including terminal states.
    #[must_use]
    pub fn mock_work_items_len(&self) -> usize {
        self.mock_work.len()
    }

    /// Acknowledges a Game Mode pause request for one tracked work item.
    pub fn confirm_mock_work_game_mode_pause(
        &mut self,
        action_id: &ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        let (before, after) = {
            let work = self.mock_work_mut(action_id)?;
            let before = work.state();
            work.confirm_game_mode_pause()
                .map_err(MockWorkError::Transition)?;
            (before, work.state())
        };

        self.audit_mock_work_transition(action_id, "confirm_game_mode_pause", Some(before), after);

        Ok(after)
    }

    /// Explicitly resumes one tracked work item after Game Mode.
    pub fn resume_mock_work(
        &mut self,
        action_id: &ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        let (before, after) = {
            let work = self.mock_work_mut(action_id)?;
            let before = work.state();
            work.resume_after_game_mode()
                .map_err(MockWorkError::Transition)?;
            (before, work.state())
        };

        self.audit_mock_work_transition(action_id, "resume_after_game_mode", Some(before), after);

        Ok(after)
    }

    /// Confirms Disabled-mode cancellation for one tracked work item.
    pub fn confirm_mock_work_cancellation(
        &mut self,
        action_id: &ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        let (before, after) = {
            let work = self.mock_work_mut(action_id)?;
            let before = work.state();
            work.confirm_cancellation()
                .map_err(MockWorkError::Transition)?;
            (before, work.state())
        };

        self.audit_mock_work_transition(
            action_id,
            "confirm_disabled_cancellation",
            Some(before),
            after,
        );

        Ok(after)
    }

    /// Records that a tracked work item cannot honor Disabled cancellation.
    pub fn mark_mock_work_cancellation_unavailable(
        &mut self,
        action_id: &ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        let (before, after) = {
            let work = self.mock_work_mut(action_id)?;
            let before = work.state();
            work.mark_cancellation_unavailable()
                .map_err(MockWorkError::Transition)?;
            (before, work.state())
        };

        self.audit_mock_work_transition(
            action_id,
            "mark_cancellation_unavailable",
            Some(before),
            after,
        );

        Ok(after)
    }

    /// Marks one tracked work item as normally completed.
    pub fn complete_mock_work(
        &mut self,
        action_id: &ActionId,
    ) -> Result<MockRunningWorkState, MockWorkError> {
        let (before, after) = {
            let work = self.mock_work_mut(action_id)?;
            let before = work.state();
            work.complete().map_err(MockWorkError::Transition)?;
            (before, work.state())
        };

        self.audit_mock_work_transition(action_id, "complete", Some(before), after);

        Ok(after)
    }

    fn mock_work_mut(
        &mut self,
        action_id: &ActionId,
    ) -> Result<&mut MockRunningWork, MockWorkError> {
        self.mock_work
            .get_mut(action_id)
            .ok_or_else(|| MockWorkError::NotTracked {
                action_id: action_id.clone(),
            })
    }

    fn audit_mock_work_transition(
        &mut self,
        action_id: &ActionId,
        operation: &'static str,
        before: Option<MockRunningWorkState>,
        after: MockRunningWorkState,
    ) {
        let before_label = before.map_or("untracked", MockRunningWorkState::as_str);

        let record = AuditRecord::new(
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
            AuditEventKind::SimulationWorkLifecycleChanged,
            "foundation-runtime",
            "Simulation-only running-work lifecycle changed",
        )
        .with_action(action_id.clone())
        .with_details(
            AuditDetails::new()
                .with_field("simulation", AuditValue::Boolean(true))
                .with_field("operation", AuditValue::Text(operation.into()))
                .with_field("from", AuditValue::Text(before_label.into()))
                .with_field("to", AuditValue::Text(after.as_str().into()))
                .with_field(
                    "changed",
                    AuditValue::Boolean(before.is_none_or(|state| state != after)),
                )
                .with_field(
                    "runtime_mode",
                    AuditValue::Text(self.runtime.mode().as_str().into()),
                )
                .with_field("terminal", AuditValue::Boolean(after.is_terminal()))
                .with_field(
                    "cooperation_pending",
                    AuditValue::Boolean(after.cooperation_pending()),
                ),
        );

        match self.audit.append(record) {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    fn emit_diagnostic(
        &mut self,
        level: DiagnosticLevel,
        component: impl Into<String>,
        message: impl Into<String>,
    ) {
        if !self.diagnostics_enabled {
            return;
        }

        match self
            .diagnostics
            .emit(DiagnosticRecord::new(level, component, message))
        {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }

    /// Evaluates one structured proposal through the mock execution boundary.
    ///
    /// Proposals requiring explicit approval are retained by the runtime until
    /// an exact matching proposal is approved successfully.
    #[must_use]
    pub fn evaluate_proposal(&mut self, proposal: &ActionProposal) -> MockExecutionOutcome {
        if let Some(pending) = self.pending_actions.get(&proposal.id)
            && pending != proposal
        {
            let superseded = pending.clone();

            self.cancel_exact_pending_action(
                &superseded,
                PendingActionCancellationReason::Superseded,
            );
        }

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

        match &outcome {
            MockExecutionOutcome::WouldExecute { action_id } => {
                self.emit_diagnostic(
                    DiagnosticLevel::Info,
                    "action",
                    format!(
                        "Action {} would execute through the mock boundary",
                        action_id.as_str()
                    ),
                );
            }
            MockExecutionOutcome::AwaitingUserApproval { action_id } => {
                self.emit_diagnostic(
                    DiagnosticLevel::Info,
                    "action",
                    format!("Action {} is awaiting user approval", action_id.as_str()),
                );
            }
            MockExecutionOutcome::Blocked { action_id, reason } => {
                self.emit_diagnostic(
                    DiagnosticLevel::Warning,
                    "action",
                    format!(
                        "Action {} was blocked by runtime safety: {reason:?}",
                        action_id.as_str()
                    ),
                );
            }
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

    /// Cancels one exact pending proposal for a system-owned lifecycle or
    /// policy reason.
    ///
    /// Cancellation is distinct from explicit user rejection and therefore
    /// uses a dedicated audit event and a runtime-owned actor.
    ///
    /// The exact stored proposal must still match, preventing stale system
    /// work from cancelling a newer replacement that reused the same action ID.
    pub fn cancel_pending_action(
        &mut self,
        proposal: &ActionProposal,
        reason: PendingActionCancellationReason,
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
        self.cancel_exact_pending_action(&pending, reason);

        Ok(())
    }

    fn cancel_exact_pending_action(
        &mut self,
        proposal: &ActionProposal,
        reason: PendingActionCancellationReason,
    ) {
        self.pending_actions.remove(&proposal.id);

        let mut record = AuditRecord::new(
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
            AuditEventKind::ActionCancelled,
            "foundation-runtime",
            "Pending action cancelled by runtime",
        )
        .with_action(proposal.id.clone())
        .with_details(
            AuditDetails::new()
                .with_field(
                    "cancellation_reason",
                    AuditValue::Text(reason.as_str().into()),
                )
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

        let event = match collector.collect() {
            Ok(event) => event,
            Err(error) => {
                self.emit_diagnostic(
                    DiagnosticLevel::Error,
                    "collector",
                    "Collector returned an error",
                );

                return Err(CollectorCycleError::Collector(error));
            }
        };

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
        collector::{
            InjectedCollectorError, MockCollector, ScriptedCollector, ScriptedCollectorStep,
        },
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

    #[test]
    fn typed_config_controls_diagnostic_emission() {
        let enabled = LychnosConfig::default();

        let enabled_runtime = FoundationRuntime::from_config(
            &enabled,
            SequenceIdProvider::default(),
            FixedTimeProvider::new(1_800_000_000_123),
        );

        assert!(enabled_runtime.diagnostics_enabled());
        assert_eq!(enabled_runtime.diagnostics_log().len(), 1);
        assert_eq!(
            enabled_runtime.diagnostics_log().records()[0].level,
            DiagnosticLevel::Info
        );
        assert_eq!(
            enabled_runtime.diagnostics_log().records()[0].component,
            "runtime"
        );

        let mut disabled = LychnosConfig::default();
        disabled.diagnostics.enabled = false;

        let disabled_runtime = FoundationRuntime::from_config(
            &disabled,
            SequenceIdProvider::default(),
            FixedTimeProvider::new(1_800_000_000_123),
        );

        assert!(!disabled_runtime.diagnostics_enabled());
        assert!(disabled_runtime.diagnostics_log().is_empty());
    }

    #[test]
    fn proposal_evaluation_emits_runtime_diagnostic() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-diagnostic");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        assert_eq!(runtime.diagnostics_log().len(), 2);

        let record = runtime
            .diagnostics_log()
            .records()
            .last()
            .expect("proposal evaluation should emit diagnostics");

        assert_eq!(record.level, DiagnosticLevel::Info);
        assert_eq!(record.component, "action");
        assert!(record.message.contains("action-diagnostic"));
        assert!(record.message.contains("awaiting user approval"));
    }

    #[test]
    fn disabling_diagnostics_does_not_disable_security_audit() {
        let mut config = LychnosConfig::default();
        config.diagnostics.enabled = false;

        let mut runtime = FoundationRuntime::from_config(
            &config,
            SequenceIdProvider::default(),
            FixedTimeProvider::new(1_800_000_000_123),
        );

        let proposal = state_changing_proposal("action-audit-still-on");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        assert!(runtime.diagnostics_log().is_empty());

        let records = runtime.audit_log().records();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, AuditEventKind::PermissionEvaluated);
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
    fn mock_work_start_respects_live_runtime_mode() {
        let mut game = runtime(RuntimeMode::GameMode);
        assert_eq!(
            game.start_mock_work(ActionId::new("work-game")),
            Err(MockWorkError::StartBlocked(RuntimeWorkStartError::GameMode))
        );

        let mut disabled = runtime(RuntimeMode::Disabled);
        assert_eq!(
            disabled.start_mock_work(ActionId::new("work-disabled")),
            Err(MockWorkError::StartBlocked(RuntimeWorkStartError::Disabled))
        );

        assert_eq!(game.mock_work_items_len(), 0);
        assert_eq!(disabled.mock_work_items_len(), 0);
    }

    #[test]
    fn mock_work_registry_tracks_pause_resume_and_disabled_cancellation() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let action_id = ActionId::new("work-lifecycle");

        assert_eq!(
            runtime.start_mock_work(action_id.clone()),
            Ok(MockRunningWorkState::Running)
        );
        assert_eq!(runtime.mock_work_items_len(), 1);

        runtime.enter_game_mode();
        assert_eq!(
            runtime.mock_work_state(&action_id),
            Some(MockRunningWorkState::PauseRequested)
        );

        assert_eq!(
            runtime.confirm_mock_work_game_mode_pause(&action_id),
            Ok(MockRunningWorkState::PausedForGameMode)
        );

        runtime.enable_normal();
        assert_eq!(
            runtime.mock_work_state(&action_id),
            Some(MockRunningWorkState::PausedForGameMode)
        );
        assert_eq!(
            runtime.resume_mock_work(&action_id),
            Ok(MockRunningWorkState::Running)
        );

        runtime.disable();
        assert_eq!(
            runtime.mock_work_state(&action_id),
            Some(MockRunningWorkState::CancellationRequested)
        );
        assert_eq!(
            runtime.confirm_mock_work_cancellation(&action_id),
            Ok(MockRunningWorkState::StoppedAfterCancellation)
        );

        runtime.enable_normal();
        assert_eq!(
            runtime.mock_work_state(&action_id),
            Some(MockRunningWorkState::StoppedAfterCancellation)
        );
    }

    #[test]
    fn mock_work_snapshot_reports_runtime_and_cooperation_state() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let action_id = ActionId::new("work-snapshot");

        runtime
            .start_mock_work(action_id.clone())
            .expect("Normal mode should permit mock work");

        runtime.enter_game_mode();

        assert_eq!(
            runtime.mock_work_snapshot(&action_id),
            Some(MockWorkLifecycleSnapshot {
                action_id: action_id.clone(),
                state: MockRunningWorkState::PauseRequested,
                runtime_mode: RuntimeMode::GameMode,
                terminal: false,
                cooperation_pending: true,
            })
        );

        runtime
            .confirm_mock_work_game_mode_pause(&action_id)
            .expect("pause acknowledgement should succeed");

        let paused = runtime
            .mock_work_snapshot(&action_id)
            .expect("tracked work should have a snapshot");

        assert_eq!(paused.state, MockRunningWorkState::PausedForGameMode);
        assert!(!paused.terminal);
        assert!(!paused.cooperation_pending);
    }

    #[test]
    fn mock_work_lifecycle_audit_is_explicitly_simulation_only() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let action_id = ActionId::new("work-audit");

        runtime
            .start_mock_work(action_id.clone())
            .expect("Normal mode should permit mock work");

        let start = runtime
            .audit_log()
            .records()
            .last()
            .expect("mock work start should be audited");

        assert_eq!(start.kind, AuditEventKind::SimulationWorkLifecycleChanged);
        assert_eq!(start.action_id.as_ref(), Some(&action_id));
        assert_eq!(
            start.details.get("simulation"),
            Some(&AuditValue::Boolean(true))
        );
        assert_eq!(
            start.details.get("operation"),
            Some(&AuditValue::Text("start".into()))
        );
        assert_eq!(
            start.details.get("from"),
            Some(&AuditValue::Text("untracked".into()))
        );
        assert_eq!(
            start.details.get("to"),
            Some(&AuditValue::Text("running".into()))
        );
        assert_eq!(
            start.details.get("runtime_mode"),
            Some(&AuditValue::Text("normal".into()))
        );

        runtime.enter_game_mode();
        runtime
            .confirm_mock_work_game_mode_pause(&action_id)
            .expect("pause acknowledgement should succeed");

        let pause = runtime
            .audit_log()
            .records()
            .last()
            .expect("pause acknowledgement should be audited");

        assert_eq!(pause.kind, AuditEventKind::SimulationWorkLifecycleChanged);
        assert_eq!(
            pause.details.get("operation"),
            Some(&AuditValue::Text("confirm_game_mode_pause".into()))
        );
        assert_eq!(
            pause.details.get("from"),
            Some(&AuditValue::Text("pause_requested".into()))
        );
        assert_eq!(
            pause.details.get("to"),
            Some(&AuditValue::Text("paused_for_game_mode".into()))
        );

        assert!(runtime.audit_log().records().iter().all(|record| !matches!(
            record.kind,
            AuditEventKind::ActionExecutionAttempted | AuditEventKind::ActionExecutionCompleted
        )));
    }

    #[test]
    fn failed_mock_work_transition_does_not_claim_lifecycle_change() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let action_id = ActionId::new("work-invalid-transition");

        runtime
            .start_mock_work(action_id.clone())
            .expect("Normal mode should permit mock work");

        let before = runtime.audit_log().len();

        assert_eq!(
            runtime.confirm_mock_work_cancellation(&action_id),
            Err(MockWorkError::Transition(
                MockRunningWorkTransitionError::CancellationNotRequested
            ))
        );

        assert_eq!(runtime.audit_log().len(), before);
    }

    #[test]
    fn mock_work_registry_rejects_duplicate_and_unknown_items() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let action_id = ActionId::new("work-duplicate");

        assert_eq!(
            runtime.start_mock_work(action_id.clone()),
            Ok(MockRunningWorkState::Running)
        );
        assert_eq!(
            runtime.start_mock_work(action_id.clone()),
            Err(MockWorkError::AlreadyTracked {
                action_id: action_id.clone(),
            })
        );

        let missing = ActionId::new("work-missing");
        assert_eq!(
            runtime.complete_mock_work(&missing),
            Err(MockWorkError::NotTracked { action_id: missing })
        );
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
    fn exact_pending_action_can_be_cancelled_by_system() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-system-cancel");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        runtime
            .cancel_pending_action(
                &proposal,
                PendingActionCancellationReason::PolicyInvalidated,
            )
            .expect("exact pending proposal should be cancellable");

        assert!(runtime.pending_action(&proposal.id).is_none());

        let last = runtime
            .audit_log()
            .records()
            .last()
            .expect("cancellation should be audited");

        assert_eq!(last.kind, AuditEventKind::ActionCancelled);
        assert_eq!(last.actor, "foundation-runtime");
        assert_eq!(
            last.details.get("cancellation_reason"),
            Some(&AuditValue::Text("policy_invalidated".into()))
        );
    }

    #[test]
    fn superseding_proposal_cancels_previous_pending_action() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let original = state_changing_proposal("action-superseded");

        assert!(matches!(
            runtime.evaluate_proposal(&original),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let replacement = ActionProposal::new(
            ActionId::new("action-superseded"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::High,
            "Replacement proposal",
            "test-analyzer",
        )
        .with_source_event(EventId::new("event-replacement"));

        assert!(matches!(
            runtime.evaluate_proposal(&replacement),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        assert_eq!(runtime.pending_action(&replacement.id), Some(&replacement));

        let cancellation = runtime
            .audit_log()
            .records()
            .iter()
            .find(|record| record.kind == AuditEventKind::ActionCancelled)
            .expect("superseded proposal should be audited as cancelled");

        assert_eq!(
            cancellation.details.get("cancellation_reason"),
            Some(&AuditValue::Text("superseded".into()))
        );

        assert_eq!(
            cancellation.event_id.as_ref().map(EventId::as_str),
            Some("event-for-action-superseded")
        );
    }

    #[test]
    fn stale_proposal_cannot_cancel_replacement() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let original = state_changing_proposal("action-cancel-stale");

        assert!(matches!(
            runtime.evaluate_proposal(&original),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let replacement = ActionProposal::new(
            ActionId::new("action-cancel-stale"),
            ActionKind::new("file.write"),
            Capability::new("file.write"),
            ActionImpact::StateChanging,
            ActionRisk::High,
            "Replacement proposal",
            "test-analyzer",
        )
        .with_source_event(EventId::new("event-cancel-replacement"));

        assert!(matches!(
            runtime.evaluate_proposal(&replacement),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        let cancellations_before = runtime
            .audit_log()
            .records()
            .iter()
            .filter(|record| record.kind == AuditEventKind::ActionCancelled)
            .count();

        let error = runtime
            .cancel_pending_action(
                &original,
                PendingActionCancellationReason::PolicyInvalidated,
            )
            .expect_err("stale proposal must not cancel its replacement");

        assert_eq!(
            error,
            PendingActionError::ProposalChanged {
                action_id: ActionId::new("action-cancel-stale"),
            }
        );

        assert_eq!(runtime.pending_action(&original.id), Some(&replacement));

        let cancellations_after = runtime
            .audit_log()
            .records()
            .iter()
            .filter(|record| record.kind == AuditEventKind::ActionCancelled)
            .count();

        assert_eq!(cancellations_after, cancellations_before);
    }

    #[test]
    fn unknown_proposal_cannot_be_cancelled() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-cancel-unknown");

        let error = runtime
            .cancel_pending_action(
                &proposal,
                PendingActionCancellationReason::PolicyInvalidated,
            )
            .expect_err("unknown proposal must not be cancellable");

        assert_eq!(
            error,
            PendingActionError::NotPending {
                action_id: ActionId::new("action-cancel-unknown"),
            }
        );

        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn system_cancellation_remains_available_while_disabled() {
        let mut runtime = runtime(RuntimeMode::Normal);
        let proposal = state_changing_proposal("action-cancel-disabled");

        assert!(matches!(
            runtime.evaluate_proposal(&proposal),
            MockExecutionOutcome::AwaitingUserApproval { .. }
        ));

        runtime.disable();

        runtime
            .cancel_pending_action(&proposal, PendingActionCancellationReason::RuntimeLifecycle)
            .expect("Disabled must not trap a system-cancelled pending action");

        assert!(runtime.pending_action(&proposal.id).is_none());

        let last = runtime
            .audit_log()
            .records()
            .last()
            .expect("runtime cancellation should be audited");

        assert_eq!(last.kind, AuditEventKind::ActionCancelled);
        assert_eq!(
            last.details.get("cancellation_reason"),
            Some(&AuditValue::Text("runtime_lifecycle".into()))
        );
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
    fn scripted_collector_failure_is_diagnosed_and_preserved() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let injected = InjectedCollectorError::new("deterministic failure");

        let mut collector =
            ScriptedCollector::from_steps([ScriptedCollectorStep::Error(injected.clone())]);

        let error = runtime
            .collect_once(&mut collector)
            .expect_err("scripted collector should fail");

        assert_eq!(error, CollectorCycleError::Collector(injected));

        let diagnostic = runtime
            .diagnostics_log()
            .records()
            .last()
            .expect("collector failure should emit a diagnostic");

        assert_eq!(diagnostic.level, DiagnosticLevel::Error);
        assert_eq!(diagnostic.component, "collector");
        assert_eq!(diagnostic.message, "Collector returned an error");

        assert!(runtime.audit_log().is_empty());
    }

    #[test]
    fn scripted_collector_recovers_after_injected_failure() {
        let mut runtime = runtime(RuntimeMode::Normal);

        let recovered_event = Event {
            id: EventId::new("event-recovered"),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_123),
            source: EventSource::new("scripted-collector"),
            kind: EventKind::new("simulation.recovered"),
            severity: Severity::Info,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        };

        let mut collector = ScriptedCollector::from_steps([
            ScriptedCollectorStep::Empty,
            ScriptedCollectorStep::Error(InjectedCollectorError::new("temporary failure")),
            ScriptedCollectorStep::Event(recovered_event),
        ]);

        assert!(
            runtime
                .collect_once(&mut collector)
                .expect("empty step should not fail")
                .is_none()
        );

        assert!(matches!(
            runtime.collect_once(&mut collector),
            Err(CollectorCycleError::Collector(_))
        ));

        let cycle = runtime
            .collect_once(&mut collector)
            .expect("collector should recover")
            .expect("recovery event should be processed");

        assert_eq!(cycle.event.id.as_str(), "event-recovered");
        assert_eq!(cycle.event.kind.as_str(), "simulation.recovered");
        assert!(matches!(
            cycle.outcome,
            MockExecutionOutcome::WouldExecute { .. }
        ));

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
