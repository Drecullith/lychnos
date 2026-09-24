//! Machine-independent orchestration for the Lychnos foundation runtime.

use std::sync::mpsc::TryRecvError;

use crate::{
    action::ActionProposal,
    analyzer::MockAnalyzer,
    audit::InMemoryAuditLog,
    bus::{EventSubscription, InMemoryEventBus, PublishReport},
    event::{Event, EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::{MockExecutionOutcome, MockExecutor},
    providers::{IdProvider, TimeProvider},
    runtime::{RuntimeController, RuntimeMode},
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
            ids,
            clock,
        }
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

    /// Processes an already-normalized event through the foundation pipeline.
    pub fn process_event(
        &mut self,
        event: Event,
    ) -> Result<FoundationCycle, FoundationRuntimeError> {
        let publish_report = self.bus.publish(event);

        let received = self.subscription.try_recv()?;

        let proposal = MockAnalyzer.analyze(&received);

        let outcome = match MockExecutor.evaluate_and_audit(
            &proposal,
            &self.runtime,
            &mut self.audit,
            self.ids.next_audit_id(),
            self.clock.audit_timestamp(),
        ) {
            Ok(outcome) => outcome,
            Err(never) => match never {},
        };

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
        action::ActionId,
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
