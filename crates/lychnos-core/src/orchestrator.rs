//! Machine-independent orchestration for the Lychnos foundation runtime.

use std::sync::mpsc::TryRecvError;

use crate::{
    action::ActionProposal,
    analyzer::MockAnalyzer,
    audit::{AuditDetails, AuditEventKind, AuditRecord, AuditSink, AuditValue, InMemoryAuditLog},
    bus::{EventSubscription, InMemoryEventBus, PublishReport},
    collector::Collector,
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
    fn disabled_runtime_blocks_collector_sourced_event() {
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
            .expect("mock collection should succeed")
            .expect("collector should produce one cycle");

        assert_eq!(
            cycle.outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-collector-event-disabled"),
                reason: DenialReason::Disabled,
            }
        );

        assert_eq!(cycle.audit_records, 1);
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
