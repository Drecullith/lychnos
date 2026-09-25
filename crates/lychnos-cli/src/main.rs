use lychnos_core::{
    config::{LychnosConfig, StartupMode},
    event::{EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::MockExecutionOutcome,
    orchestrator::FoundationRuntime,
    providers::{SequenceIdProvider, SystemTimeProvider},
};

fn main() {
    println!(
        "{} core v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    let config = requested_config();

    let mut runtime =
        FoundationRuntime::from_config(&config, SequenceIdProvider::default(), SystemTimeProvider);

    println!(
        "Starting foundation simulation in {:?} mode...",
        runtime.mode()
    );

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

fn requested_config() -> LychnosConfig {
    let mut config = LychnosConfig::default();

    config.runtime.startup_mode = match std::env::args().nth(1).as_deref() {
        Some("game") => StartupMode::GameMode,
        Some("disabled") => StartupMode::Disabled,
        _ => StartupMode::Normal,
    };

    config
}
