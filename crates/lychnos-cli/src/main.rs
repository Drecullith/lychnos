use lychnos_core::{
    analyzer::MockAnalyzer,
    audit::{AuditId, AuditTimestamp, InMemoryAuditLog},
    bus::InMemoryEventBus,
    event::{
        Event, EventId, EventKind, EventPayload, EventSource, EventTimestamp, Sensitivity, Severity,
    },
    executor::{MockExecutionOutcome, MockExecutor},
    runtime::{RuntimeController, RuntimeMode},
};

fn main() {
    println!(
        "{} core v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    let mode = requested_mode();

    println!("Starting foundation simulation in {mode:?} mode...");

    let bus = InMemoryEventBus::new();
    let subscription = bus.subscribe();

    let event = Event {
        id: EventId::new("cli-event-001"),
        occurred_at: EventTimestamp::from_unix_millis(1_800_000_000_000),
        source: EventSource::new("lychnos-cli"),
        kind: EventKind::new("simulation.event"),
        severity: Severity::Info,
        sensitivity: Sensitivity::Standard,
        correlation_id: None,
        payload: EventPayload::new(),
    };

    let report = bus.publish(event);

    println!(
        "Event published: delivered={}, disconnected={}",
        report.delivered, report.disconnected
    );

    let received = subscription
        .try_recv()
        .expect("simulation event should reach the subscriber");

    println!(
        "Event received: {} ({})",
        received.id.as_str(),
        received.kind.as_str()
    );

    let proposal = MockAnalyzer.analyze(&received);

    println!(
        "Action proposed: {} ({})",
        proposal.id.as_str(),
        proposal.kind.as_str()
    );

    let runtime = RuntimeController::new(mode);
    let mut audit = InMemoryAuditLog::new();

    let outcome = MockExecutor
        .evaluate_and_audit(
            &proposal,
            &runtime,
            &mut audit,
            AuditId::new("cli-audit-001"),
            AuditTimestamp::from_unix_millis(1_800_000_000_001),
        )
        .expect("in-memory audit append cannot fail");

    match outcome {
        MockExecutionOutcome::WouldExecute { .. } => {
            println!("Decision: WOULD EXECUTE (mock only)");
        }
        MockExecutionOutcome::AwaitingUserApproval { .. } => {
            println!("Decision: AWAITING USER APPROVAL");
        }
        MockExecutionOutcome::Blocked { reason, .. } => {
            println!("Decision: BLOCKED ({reason:?})");
        }
    }

    println!("Audit records written: {}", audit.len());
}

fn requested_mode() -> RuntimeMode {
    match std::env::args().nth(1).as_deref() {
        Some("game") => RuntimeMode::GameMode,
        Some("disabled") => RuntimeMode::Disabled,
        _ => RuntimeMode::Normal,
    }
}
