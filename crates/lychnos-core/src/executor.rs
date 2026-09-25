//! Safe mock execution boundary for foundation-phase Lychnos.

use crate::{
    action::{ActionId, ActionProposal},
    approval::ApprovalGrant,
    audit::{
        AuditDetails, AuditEventKind, AuditId, AuditRecord, AuditSink, AuditTimestamp, AuditValue,
    },
    permission::{DefaultPermissionPolicy, DenialReason, PermissionDecision},
    runtime::{RuntimeController, RuntimeWorkLease, RuntimeWorkRequest},
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

/// Observable state of one already-started mock operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockRunningWorkState {
    /// Work is active and no pause or cancellation has been requested.
    Running,
    /// Game Mode requested cooperative pause/quiescence, but the simulated
    /// executor has not acknowledged the pause yet.
    PauseRequested,
    /// The simulated executor acknowledged the Game Mode pause request.
    PausedForGameMode,
    /// Disabled-mode safety requested cooperative cancellation, but the
    /// simulated executor has not confirmed that work stopped.
    CancellationRequested,
    /// The simulated executor reported that the running work cannot honor the
    /// cancellation request and is still active.
    CancellationUnavailable,
    /// Work reached normal completion.
    Completed,
    /// The simulated executor confirmed that work stopped because of the
    /// cancellation request.
    StoppedAfterCancellation,
}

/// Invalid simulated running-work lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockRunningWorkTransitionError {
    /// A Game Mode pause acknowledgement was attempted without a live pause
    /// request.
    PauseNotRequested,
    /// Resume was attempted while Game Mode still requests a pause.
    GameModePauseStillRequested,
    /// Resume was attempted for work that was not paused.
    NotPaused,
    /// Disabled cancellation now takes precedence over the requested
    /// transition.
    CancellationRequested,
    /// A cancellation-specific transition was requested before runtime safety
    /// requested cancellation.
    CancellationNotRequested,
    /// Work is paused and cannot complete until explicitly resumed.
    WorkPaused,
    /// Work already reached a terminal state.
    AlreadyTerminal,
    /// The executor previously reported that cancellation cannot be honored.
    CancellationUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MockRunningWorkLifecycle {
    Active,
    PausedForGameMode,
    CancellationUnavailable,
    Completed,
    StoppedAfterCancellation,
}

/// Simulation-only representation of action work that has already started.
///
/// This type performs no operating-system action. It exists so Phase 2 can
/// define cancellation semantics before a real executor is introduced.
#[derive(Debug, Clone)]
pub struct MockRunningWork {
    action_id: ActionId,
    lease: RuntimeWorkLease,
    lifecycle: MockRunningWorkLifecycle,
}

impl MockRunningWork {
    /// Creates a simulation-only running-work representation from a
    /// runtime-issued lease.
    ///
    /// This does not perform operating-system work or grant execution
    /// authority. It only models the lifecycle of work that has already
    /// started so cancellation behavior can be tested before a real executor
    /// exists.
    #[must_use]
    pub fn new(action_id: ActionId, lease: RuntimeWorkLease) -> Self {
        Self {
            action_id,
            lease,
            lifecycle: MockRunningWorkLifecycle::Active,
        }
    }

    /// Returns the associated action identifier.
    #[must_use]
    pub fn action_id(&self) -> &ActionId {
        &self.action_id
    }

    /// Returns the current simulated running-work state.
    ///
    /// A runtime cancellation request is observable immediately, but it does
    /// not become a confirmed stop until the simulated executor explicitly
    /// acknowledges it.
    #[must_use]
    pub fn state(&self) -> MockRunningWorkState {
        match self.lifecycle {
            MockRunningWorkLifecycle::Active => match self.lease.request() {
                RuntimeWorkRequest::Continue => MockRunningWorkState::Running,
                RuntimeWorkRequest::PauseForGameMode => MockRunningWorkState::PauseRequested,
                RuntimeWorkRequest::CancelForDisabled => {
                    MockRunningWorkState::CancellationRequested
                }
            },
            MockRunningWorkLifecycle::PausedForGameMode => {
                if self.lease.request() == RuntimeWorkRequest::CancelForDisabled {
                    MockRunningWorkState::CancellationRequested
                } else {
                    MockRunningWorkState::PausedForGameMode
                }
            }
            MockRunningWorkLifecycle::CancellationUnavailable => {
                MockRunningWorkState::CancellationUnavailable
            }
            MockRunningWorkLifecycle::Completed => MockRunningWorkState::Completed,
            MockRunningWorkLifecycle::StoppedAfterCancellation => {
                MockRunningWorkState::StoppedAfterCancellation
            }
        }
    }

    /// Acknowledges a live Game Mode pause request.
    ///
    /// Pausing is cooperative and reversible. Returning the runtime to Normal
    /// permits an explicit resume but does not silently resume already-paused
    /// work.
    pub fn confirm_game_mode_pause(&mut self) -> Result<(), MockRunningWorkTransitionError> {
        match self.lifecycle {
            MockRunningWorkLifecycle::Active => match self.lease.request() {
                RuntimeWorkRequest::PauseForGameMode => {
                    self.lifecycle = MockRunningWorkLifecycle::PausedForGameMode;
                    Ok(())
                }
                RuntimeWorkRequest::CancelForDisabled => {
                    Err(MockRunningWorkTransitionError::CancellationRequested)
                }
                RuntimeWorkRequest::Continue => {
                    Err(MockRunningWorkTransitionError::PauseNotRequested)
                }
            },
            MockRunningWorkLifecycle::PausedForGameMode => Ok(()),
            MockRunningWorkLifecycle::CancellationUnavailable => {
                Err(MockRunningWorkTransitionError::CancellationUnavailable)
            }
            MockRunningWorkLifecycle::Completed
            | MockRunningWorkLifecycle::StoppedAfterCancellation => {
                Err(MockRunningWorkTransitionError::AlreadyTerminal)
            }
        }
    }

    /// Explicitly resumes work previously paused for Game Mode.
    pub fn resume_after_game_mode(&mut self) -> Result<(), MockRunningWorkTransitionError> {
        match self.lifecycle {
            MockRunningWorkLifecycle::PausedForGameMode => match self.lease.request() {
                RuntimeWorkRequest::Continue => {
                    self.lifecycle = MockRunningWorkLifecycle::Active;
                    Ok(())
                }
                RuntimeWorkRequest::PauseForGameMode => {
                    Err(MockRunningWorkTransitionError::GameModePauseStillRequested)
                }
                RuntimeWorkRequest::CancelForDisabled => {
                    Err(MockRunningWorkTransitionError::CancellationRequested)
                }
            },
            MockRunningWorkLifecycle::Active => Err(MockRunningWorkTransitionError::NotPaused),
            MockRunningWorkLifecycle::CancellationUnavailable => {
                Err(MockRunningWorkTransitionError::CancellationUnavailable)
            }
            MockRunningWorkLifecycle::Completed
            | MockRunningWorkLifecycle::StoppedAfterCancellation => {
                Err(MockRunningWorkTransitionError::AlreadyTerminal)
            }
        }
    }

    /// Marks the simulated work as normally completed.
    ///
    /// Normal completion is allowed even after cancellation was requested,
    /// modelling the race where work finishes before cooperative cancellation
    /// can take effect.
    pub fn complete(&mut self) -> Result<(), MockRunningWorkTransitionError> {
        match self.lifecycle {
            MockRunningWorkLifecycle::Active
            | MockRunningWorkLifecycle::CancellationUnavailable => {
                self.lifecycle = MockRunningWorkLifecycle::Completed;
                Ok(())
            }
            MockRunningWorkLifecycle::PausedForGameMode => {
                Err(MockRunningWorkTransitionError::WorkPaused)
            }
            MockRunningWorkLifecycle::Completed
            | MockRunningWorkLifecycle::StoppedAfterCancellation => {
                Err(MockRunningWorkTransitionError::AlreadyTerminal)
            }
        }
    }

    /// Confirms that the simulated executor stopped work because cancellation
    /// had been requested.
    ///
    /// Merely entering Disabled never calls this automatically: the runtime
    /// requests cancellation, while the executor owns acknowledgement.
    pub fn confirm_cancellation(&mut self) -> Result<(), MockRunningWorkTransitionError> {
        match self.lifecycle {
            MockRunningWorkLifecycle::Active | MockRunningWorkLifecycle::PausedForGameMode
                if self.lease.cancellation_requested() =>
            {
                self.lifecycle = MockRunningWorkLifecycle::StoppedAfterCancellation;
                Ok(())
            }
            MockRunningWorkLifecycle::Active | MockRunningWorkLifecycle::PausedForGameMode => {
                Err(MockRunningWorkTransitionError::CancellationNotRequested)
            }
            MockRunningWorkLifecycle::CancellationUnavailable => {
                Err(MockRunningWorkTransitionError::CancellationUnavailable)
            }
            MockRunningWorkLifecycle::Completed
            | MockRunningWorkLifecycle::StoppedAfterCancellation => {
                Err(MockRunningWorkTransitionError::AlreadyTerminal)
            }
        }
    }

    /// Records that the simulated executor cannot honor the current
    /// cancellation request while work remains active.
    pub fn mark_cancellation_unavailable(&mut self) -> Result<(), MockRunningWorkTransitionError> {
        match self.lifecycle {
            MockRunningWorkLifecycle::Active | MockRunningWorkLifecycle::PausedForGameMode
                if self.lease.cancellation_requested() =>
            {
                self.lifecycle = MockRunningWorkLifecycle::CancellationUnavailable;
                Ok(())
            }
            MockRunningWorkLifecycle::Active | MockRunningWorkLifecycle::PausedForGameMode => {
                Err(MockRunningWorkTransitionError::CancellationNotRequested)
            }
            MockRunningWorkLifecycle::CancellationUnavailable => Ok(()),
            MockRunningWorkLifecycle::Completed
            | MockRunningWorkLifecycle::StoppedAfterCancellation => {
                Err(MockRunningWorkTransitionError::AlreadyTerminal)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalState {
    NotRequired,
    Missing,
    Matched,
    Mismatched,
    RuntimeBlocked,
}

impl ApprovalState {
    const fn audit_label(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Missing => "missing",
            Self::Matched => "matched",
            Self::Mismatched => "mismatched",
            Self::RuntimeBlocked => "runtime_blocked",
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
        self.evaluate_internal(proposal, runtime, None).0
    }

    /// Evaluates an action using one explicit approval grant.
    ///
    /// The grant is consumed by this call. Runtime safety is evaluated before
    /// approval matching, so a grant cannot bypass Game Mode or Disabled.
    #[must_use]
    pub fn evaluate_with_approval(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
        approval: ApprovalGrant,
    ) -> MockExecutionOutcome {
        self.evaluate_internal(proposal, runtime, Some(&approval)).0
    }

    fn evaluate_internal(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
        approval: Option<&ApprovalGrant>,
    ) -> (MockExecutionOutcome, ApprovalState) {
        match DefaultPermissionPolicy.evaluate(proposal, runtime) {
            PermissionDecision::Allowed => (
                MockExecutionOutcome::WouldExecute {
                    action_id: proposal.id.clone(),
                },
                ApprovalState::NotRequired,
            ),

            PermissionDecision::RequiresUserApproval => match approval {
                Some(grant) if grant.matches(proposal) => (
                    MockExecutionOutcome::WouldExecute {
                        action_id: proposal.id.clone(),
                    },
                    ApprovalState::Matched,
                ),

                Some(_) => (
                    MockExecutionOutcome::AwaitingUserApproval {
                        action_id: proposal.id.clone(),
                    },
                    ApprovalState::Mismatched,
                ),

                None => (
                    MockExecutionOutcome::AwaitingUserApproval {
                        action_id: proposal.id.clone(),
                    },
                    ApprovalState::Missing,
                ),
            },

            PermissionDecision::Denied(reason) => (
                MockExecutionOutcome::Blocked {
                    action_id: proposal.id.clone(),
                    reason,
                },
                ApprovalState::RuntimeBlocked,
            ),
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
        self.evaluate_and_audit_internal(proposal, runtime, None, audit, audit_id, occurred_at)
    }

    /// Evaluates an action with explicit approval and audits the result.
    pub fn evaluate_with_approval_and_audit<S: AuditSink>(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
        approval: ApprovalGrant,
        audit: &mut S,
        audit_id: AuditId,
        occurred_at: AuditTimestamp,
    ) -> Result<MockExecutionOutcome, S::Error> {
        self.evaluate_and_audit_internal(
            proposal,
            runtime,
            Some(&approval),
            audit,
            audit_id,
            occurred_at,
        )
    }

    fn evaluate_and_audit_internal<S: AuditSink>(
        self,
        proposal: &ActionProposal,
        runtime: &RuntimeController,
        approval: Option<&ApprovalGrant>,
        audit: &mut S,
        audit_id: AuditId,
        occurred_at: AuditTimestamp,
    ) -> Result<MockExecutionOutcome, S::Error> {
        let (outcome, approval_state) = self.evaluate_internal(proposal, runtime, approval);

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
                .with_field("decision", AuditValue::Text(outcome.audit_label().into()))
                .with_field(
                    "approval",
                    AuditValue::Text(approval_state.audit_label().into()),
                ),
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
    fn exact_approval_allows_state_changing_action() {
        let runtime = RuntimeController::default();
        let proposal = proposal("action-008", ActionImpact::StateChanging);
        let approval = ApprovalGrant::new(&proposal, "user");

        let outcome = MockExecutor.evaluate_with_approval(&proposal, &runtime, approval);

        assert_eq!(
            outcome,
            MockExecutionOutcome::WouldExecute {
                action_id: ActionId::new("action-008"),
            }
        );
    }

    #[test]
    fn mismatched_approval_does_not_authorize_action() {
        let runtime = RuntimeController::default();

        let original = proposal("action-009", ActionImpact::StateChanging);
        let approval = ApprovalGrant::new(&original, "user");

        let changed = ActionProposal::new(
            ActionId::new("action-009"),
            ActionKind::new("mock.changed"),
            Capability::new("mock.capability"),
            ActionImpact::StateChanging,
            ActionRisk::Low,
            "Changed after approval",
            "test",
        );

        let outcome = MockExecutor.evaluate_with_approval(&changed, &runtime, approval);

        assert_eq!(
            outcome,
            MockExecutionOutcome::AwaitingUserApproval {
                action_id: ActionId::new("action-009"),
            }
        );
    }

    #[test]
    fn runtime_disable_overrides_existing_approval() {
        let runtime = RuntimeController::default();
        let proposal = proposal("action-010", ActionImpact::StateChanging);
        let approval = ApprovalGrant::new(&proposal, "user");

        runtime.disable();

        let outcome = MockExecutor.evaluate_with_approval(&proposal, &runtime, approval);

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-010"),
                reason: DenialReason::Disabled,
            }
        );
    }

    #[test]
    fn matched_approval_is_recorded_in_security_audit() {
        let runtime = RuntimeController::default();
        let mut audit = InMemoryAuditLog::new();

        let proposal = proposal("action-011", ActionImpact::StateChanging);
        let approval = ApprovalGrant::new(&proposal, "user");

        MockExecutor
            .evaluate_with_approval_and_audit(
                &proposal,
                &runtime,
                approval,
                &mut audit,
                AuditId::new("audit-011"),
                AuditTimestamp::from_unix_millis(11),
            )
            .expect("in-memory audit append cannot fail");

        assert_eq!(
            audit.records()[0].details.get("approval"),
            Some(&AuditValue::Text("matched".into()))
        );
        assert_eq!(
            audit.records()[0].details.get("decision"),
            Some(&AuditValue::Text("would_execute".into()))
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

    #[test]
    fn already_started_mock_work_observes_disable_cancellation() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");

        let work = MockRunningWork::new(ActionId::new("running-001"), lease);

        assert_eq!(work.action_id().as_str(), "running-001");
        assert_eq!(work.state(), MockRunningWorkState::Running);

        runtime.disable();

        assert_eq!(work.state(), MockRunningWorkState::CancellationRequested);
    }

    #[test]
    fn cancelled_mock_work_does_not_resume_after_reenable() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");

        let work = MockRunningWork::new(ActionId::new("running-002"), lease);

        runtime.disable();
        runtime.enable_normal();

        assert_eq!(runtime.mode(), crate::runtime::RuntimeMode::Normal);
        assert_eq!(work.state(), MockRunningWorkState::CancellationRequested);
    }

    #[test]
    fn new_mock_work_after_reenable_starts_uncancelled() {
        let runtime = RuntimeController::default();

        let old_lease = runtime
            .try_begin_work()
            .expect("first simulated work should start");
        let old_work = MockRunningWork::new(ActionId::new("running-old"), old_lease);

        runtime.disable();
        runtime.enable_normal();

        let new_lease = runtime
            .try_begin_work()
            .expect("new work should start after explicit re-enable");
        let new_work = MockRunningWork::new(ActionId::new("running-new"), new_lease);

        assert_eq!(
            old_work.state(),
            MockRunningWorkState::CancellationRequested
        );
        assert_eq!(new_work.state(), MockRunningWorkState::Running);
    }

    #[test]
    fn cancellation_request_is_not_a_confirmed_stop() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-requested"), lease);

        runtime.disable();

        assert_eq!(work.state(), MockRunningWorkState::CancellationRequested);

        work.confirm_cancellation()
            .expect("executor may confirm a requested cancellation");

        assert_eq!(work.state(), MockRunningWorkState::StoppedAfterCancellation);
    }

    #[test]
    fn cancellation_cannot_be_confirmed_before_request() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-no-request"), lease);

        assert_eq!(
            work.confirm_cancellation(),
            Err(MockRunningWorkTransitionError::CancellationNotRequested)
        );
        assert_eq!(work.state(), MockRunningWorkState::Running);
    }

    #[test]
    fn work_may_complete_after_cancellation_request_race() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-race"), lease);

        runtime.disable();
        assert_eq!(work.state(), MockRunningWorkState::CancellationRequested);

        work.complete()
            .expect("work may finish before cooperative cancellation takes effect");

        assert_eq!(work.state(), MockRunningWorkState::Completed);
        assert_eq!(
            work.confirm_cancellation(),
            Err(MockRunningWorkTransitionError::AlreadyTerminal)
        );
    }

    #[test]
    fn cancellation_unavailable_is_distinct_from_requested_and_stopped() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-unavailable"), lease);

        runtime.disable();

        work.mark_cancellation_unavailable()
            .expect("executor may report inability to honor a cancellation request");

        assert_eq!(work.state(), MockRunningWorkState::CancellationUnavailable);
        assert_eq!(
            work.confirm_cancellation(),
            Err(MockRunningWorkTransitionError::CancellationUnavailable)
        );

        work.complete()
            .expect("uncancellable work may still later complete normally");
        assert_eq!(work.state(), MockRunningWorkState::Completed);
    }

    #[test]
    fn game_mode_pause_request_requires_executor_acknowledgement() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-game-pause"), lease);

        runtime.enter_game_mode();

        assert_eq!(work.state(), MockRunningWorkState::PauseRequested);

        work.confirm_game_mode_pause()
            .expect("executor may acknowledge a live Game Mode pause request");

        assert_eq!(work.state(), MockRunningWorkState::PausedForGameMode);
        assert_eq!(
            work.complete(),
            Err(MockRunningWorkTransitionError::WorkPaused)
        );
    }

    #[test]
    fn paused_work_requires_explicit_resume_after_game_mode() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-game-resume"), lease);

        runtime.enter_game_mode();
        work.confirm_game_mode_pause()
            .expect("Game Mode pause should be acknowledged");

        runtime.enable_normal();

        assert_eq!(work.state(), MockRunningWorkState::PausedForGameMode);

        work.resume_after_game_mode()
            .expect("Normal mode permits explicit resume");

        assert_eq!(work.state(), MockRunningWorkState::Running);
    }

    #[test]
    fn paused_work_cannot_resume_while_game_mode_still_active() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-game-still"), lease);

        runtime.enter_game_mode();
        work.confirm_game_mode_pause()
            .expect("Game Mode pause should be acknowledged");

        assert_eq!(
            work.resume_after_game_mode(),
            Err(MockRunningWorkTransitionError::GameModePauseStillRequested)
        );
        assert_eq!(work.state(), MockRunningWorkState::PausedForGameMode);
    }

    #[test]
    fn disabled_upgrades_paused_work_to_irreversible_cancellation() {
        let runtime = RuntimeController::default();
        let lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut work = MockRunningWork::new(ActionId::new("running-game-disabled"), lease);

        runtime.enter_game_mode();
        work.confirm_game_mode_pause()
            .expect("Game Mode pause should be acknowledged");

        runtime.disable();

        assert_eq!(work.state(), MockRunningWorkState::CancellationRequested);
        assert_eq!(
            work.resume_after_game_mode(),
            Err(MockRunningWorkTransitionError::CancellationRequested)
        );

        work.confirm_cancellation()
            .expect("Disabled cancellation should be confirmable from paused work");

        runtime.enable_normal();

        assert_eq!(work.state(), MockRunningWorkState::StoppedAfterCancellation);
    }

    #[test]
    fn terminal_running_work_states_do_not_revive_after_reenable() {
        let runtime = RuntimeController::default();

        let completed_lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut completed =
            MockRunningWork::new(ActionId::new("running-completed"), completed_lease);
        completed
            .complete()
            .expect("simulated work should complete once");

        let cancelled_lease = runtime
            .try_begin_work()
            .expect("Normal mode should permit simulated work start");
        let mut cancelled =
            MockRunningWork::new(ActionId::new("running-cancelled"), cancelled_lease);

        runtime.disable();
        cancelled
            .confirm_cancellation()
            .expect("requested cancellation should be confirmable");
        runtime.enable_normal();

        assert_eq!(completed.state(), MockRunningWorkState::Completed);
        assert_eq!(
            cancelled.state(),
            MockRunningWorkState::StoppedAfterCancellation
        );
        assert_eq!(
            completed.complete(),
            Err(MockRunningWorkTransitionError::AlreadyTerminal)
        );
        assert_eq!(
            cancelled.complete(),
            Err(MockRunningWorkTransitionError::AlreadyTerminal)
        );
    }
}
