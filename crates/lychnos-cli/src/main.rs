use std::{fs, path::PathBuf, thread, time::Duration};

use lychnos_core::{
    action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
    config::{LychnosConfig, StartupMode},
    event::{EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::MockExecutionOutcome,
    orchestrator::FoundationRuntime,
    presentation::CompanionPresentationEnvelope,
    providers::{SequenceIdProvider, SystemTimeProvider},
};

type Runtime = FoundationRuntime<SequenceIdProvider, SystemTimeProvider>;

fn main() {
    println!(
        "{} core v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    if std::env::args().nth(1).as_deref() == Some("presentation-demo") {
        run_presentation_demo();
        return;
    }

    run_foundation_simulation();
}

fn run_foundation_simulation() {
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

fn run_presentation_demo() {
    let mut runtime = FoundationRuntime::new(
        Default::default(),
        SequenceIdProvider::default(),
        SystemTimeProvider,
    );

    println!(
        "Publishing live read-only presentation snapshots to {}",
        presentation_snapshot_path().display()
    );

    publish_snapshot(&runtime, "idle");
    pause_demo();

    let work_id = ActionId::new("presentation-demo-work");
    runtime
        .start_mock_work(work_id.clone())
        .expect("normal runtime should start mock work");
    publish_snapshot(&runtime, "working");
    pause_demo();

    runtime
        .complete_mock_work(&work_id)
        .expect("demo work should complete");

    let proposal = ActionProposal::new(
        ActionId::new("presentation-demo-approval"),
        ActionKind::new("demo.state_change"),
        Capability::new("demo.state_change"),
        ActionImpact::StateChanging,
        ActionRisk::Moderate,
        "Allow the simulated state-changing action",
        "presentation-demo",
    );

    let outcome = runtime.evaluate_proposal(&proposal);
    assert!(matches!(
        outcome,
        MockExecutionOutcome::AwaitingUserApproval { .. }
    ));
    publish_snapshot(&runtime, "approval");
    pause_demo();

    runtime
        .reject_pending_action(&proposal, "presentation-demo-user")
        .expect("demo proposal should still be pending");

    runtime.enter_game_mode();
    publish_snapshot(&runtime, "game_mode");
    pause_demo();

    runtime.disable();
    publish_snapshot(&runtime, "disabled");
    pause_demo();

    runtime.enable_normal();
    publish_snapshot(&runtime, "normal");
    println!("Presentation demo complete; final snapshot is Normal.");
}

fn pause_demo() {
    thread::sleep(Duration::from_secs(3));
}

fn publish_snapshot(runtime: &Runtime, label: &str) {
    let path = presentation_snapshot_path();
    let envelope = CompanionPresentationEnvelope::new(runtime.presentation_state());
    let json = envelope
        .to_json()
        .expect("presentation envelope should serialize");

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("presentation snapshot directory should be writable");
    }

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, format!("{json}\n"))
        .expect("presentation snapshot temporary file should be writable");

    if let Err(error) = fs::rename(&temporary, &path) {
        if path.exists() {
            fs::remove_file(&path).expect("existing presentation snapshot should be removable");
            fs::rename(&temporary, &path)
                .expect("presentation snapshot should replace previous snapshot");
        } else {
            panic!("failed to publish presentation snapshot: {error}");
        }
    }

    println!(
        "presentation={label:<10} mode={:<9} approvals={} active_work={}",
        runtime.mode().as_str(),
        runtime.pending_actions_len(),
        runtime
            .presentation_state()
            .tracked_work
            .iter()
            .filter(|work| !work.terminal)
            .count()
    );
}

fn presentation_snapshot_path() -> PathBuf {
    if let Ok(path) = std::env::var("LYCHNOS_PRESENTATION_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("lychnos/presentation-v1.json");
    }

    std::env::temp_dir().join("lychnos/presentation-v1.json")
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
