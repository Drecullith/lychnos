//! Deterministic mock analysis used by foundation-phase Lychnos tests.

use crate::{
    action::{
        ActionId, ActionImpact, ActionKind, ActionParameters, ActionProposal, ActionRisk,
        ActionValue, Capability,
    },
    event::Event,
};

/// Foundation-phase analyzer that creates deterministic read-only proposals.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockAnalyzer;

impl MockAnalyzer {
    /// Converts one normalized event into a safe structured action proposal.
    #[must_use]
    pub fn analyze(self, event: &Event) -> ActionProposal {
        ActionProposal::new(
            ActionId::new(format!("action-for-{}", event.id.as_str())),
            ActionKind::new("event.inspect"),
            Capability::new("event.read"),
            ActionImpact::ReadOnly,
            ActionRisk::Low,
            format!("Inspect normalized event {}", event.kind.as_str()),
            "mock-analyzer",
        )
        .with_parameters(
            ActionParameters::new()
                .with_field("source", ActionValue::Text(event.source.as_str().into()))
                .with_field("kind", ActionValue::Text(event.kind.as_str().into())),
        )
        .with_source_event(event.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audit::{AuditId, AuditTimestamp, AuditValue, InMemoryAuditLog},
        bus::InMemoryEventBus,
        event::{
            Event, EventId, EventKind, EventPayload, EventSource, EventTimestamp, Sensitivity,
            Severity,
        },
        executor::{MockExecutionOutcome, MockExecutor},
        permission::DenialReason,
        runtime::RuntimeController,
    };

    fn mock_event(id: &str) -> Event {
        Event {
            id: EventId::new(id),
            occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_000),
            source: EventSource::new("mock-terminal"),
            kind: EventKind::new("terminal.command_failed"),
            severity: Severity::Error,
            sensitivity: Sensitivity::Standard,
            correlation_id: None,
            payload: EventPayload::new(),
        }
    }

    #[test]
    fn analyzer_links_action_to_source_event() {
        let event = mock_event("event-001");

        let proposal = MockAnalyzer.analyze(&event);

        assert_eq!(proposal.id.as_str(), "action-for-event-001");
        assert_eq!(proposal.kind.as_str(), "event.inspect");
        assert_eq!(proposal.capability.as_str(), "event.read");

        assert_eq!(
            proposal.source_event_id.as_ref().map(EventId::as_str),
            Some("event-001")
        );

        assert_eq!(
            proposal.parameters.get("source"),
            Some(&ActionValue::Text("mock-terminal".into()))
        );
    }

    #[test]
    fn normal_pipeline_flows_from_event_bus_to_audit() {
        let bus = InMemoryEventBus::new();
        let subscription = bus.subscribe();

        let publish_report = bus.publish(mock_event("event-002"));

        assert_eq!(publish_report.delivered, 1);

        let event = subscription
            .try_recv()
            .expect("published event should reach subscriber");

        let proposal = MockAnalyzer.analyze(&event);

        let runtime = RuntimeController::default();
        let mut audit = InMemoryAuditLog::new();

        let outcome = MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-002"),
                AuditTimestamp::from_unix_millis(2),
            )
            .expect("in-memory audit append cannot fail");

        assert!(matches!(outcome, MockExecutionOutcome::WouldExecute { .. }));

        assert_eq!(audit.len(), 1);

        let record = &audit.records()[0];

        assert_eq!(
            record.event_id.as_ref().map(EventId::as_str),
            Some("event-002")
        );

        assert_eq!(
            record.details.get("decision"),
            Some(&AuditValue::Text("would_execute".into()))
        );
    }

    #[test]
    fn game_mode_blocks_foundation_pipeline() {
        let bus = InMemoryEventBus::new();
        let subscription = bus.subscribe();

        bus.publish(mock_event("event-003"));

        let event = subscription
            .try_recv()
            .expect("published event should reach subscriber");

        let proposal = MockAnalyzer.analyze(&event);

        let runtime = RuntimeController::default();
        runtime.enter_game_mode();

        let mut audit = InMemoryAuditLog::new();

        let outcome = MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-003"),
                AuditTimestamp::from_unix_millis(3),
            )
            .expect("in-memory audit append cannot fail");

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-event-003"),
                reason: DenialReason::GameMode,
            }
        );

        assert_eq!(
            audit.records()[0].details.get("decision"),
            Some(&AuditValue::Text("blocked_game_mode".into()))
        );
    }

    #[test]
    fn disabling_after_analysis_still_blocks_execution_boundary() {
        let bus = InMemoryEventBus::new();
        let subscription = bus.subscribe();

        bus.publish(mock_event("event-004"));

        let event = subscription
            .try_recv()
            .expect("published event should reach subscriber");

        let proposal = MockAnalyzer.analyze(&event);

        let runtime = RuntimeController::default();

        assert!(runtime.actions_allowed());

        runtime.disable();

        let mut audit = InMemoryAuditLog::new();

        let outcome = MockExecutor
            .evaluate_and_audit(
                &proposal,
                &runtime,
                &mut audit,
                AuditId::new("audit-004"),
                AuditTimestamp::from_unix_millis(4),
            )
            .expect("in-memory audit append cannot fail");

        assert_eq!(
            outcome,
            MockExecutionOutcome::Blocked {
                action_id: ActionId::new("action-for-event-004"),
                reason: DenialReason::Disabled,
            }
        );

        assert_eq!(
            audit.records()[0].details.get("decision"),
            Some(&AuditValue::Text("blocked_disabled".into()))
        );
    }
}
