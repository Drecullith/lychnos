use lychnos_core::{
    event::{EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::MockExecutionOutcome,
    orchestrator::FoundationRuntime,
    providers::{SequenceIdProvider, SystemTimeProvider},
    runtime::RuntimeMode,
};

fn main() {
    println!(
        "{} core v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    let mode = requested_mode();

    println!("Starting foundation simulation in {mode:?} mode...");

    let mut runtime =
        FoundationRuntime::new(mode, SequenceIdProvider::default(), SystemTimeProvider);

    let cycle = runtime
        .observe(
            EventSource::new("lychnos-cli"),
            EventKind::new("simulation.event"),
            Severity::Info,
            Sensitivity::Standard,
            EventPayload::new(),
        )
        .expect("foundation simulation should process");

    println!(
        "Event published: delivered={}, disconnected={}",
        cycle.publish_report.delivered, cycle.publish_report.disconnected
    );

    println!(
        "Event received: {} ({})",
        cycle.event.id.as_str(),
        cycle.event.kind.as_str()
    );

    println!(
        "Action proposed: {} ({})",
        cycle.proposal.id.as_str(),
        cycle.proposal.kind.as_str()
    );

    match &cycle.outcome {
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

    println!("Audit records written: {}", cycle.audit_records);
}

fn requested_mode() -> RuntimeMode {
    match std::env::args().nth(1).as_deref() {
        Some("game") => RuntimeMode::GameMode,
        Some("disabled") => RuntimeMode::Disabled,
        _ => RuntimeMode::Normal,
    }
}
