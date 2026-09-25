use std::{fs, path::PathBuf, thread, time::Duration};

use lychnos_core::{
    action::{ActionId, ActionImpact, ActionKind, ActionProposal, ActionRisk, Capability},
    config::{LychnosConfig, StartupMode},
    event::{EventKind, EventPayload, EventSource, Sensitivity, Severity},
    executor::MockExecutionOutcome,
    orchestrator::FoundationRuntime,
    presentation::{
        CompanionControlEnvelope, CompanionPresentationEnvelope, PendingApprovalDecision,
    },
    providers::{SequenceIdProvider, SystemTimeProvider},
};

type Runtime = FoundationRuntime<SequenceIdProvider, SystemTimeProvider>;

fn main() {
    println!(
        "{} core v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    match std::env::args().nth(1).as_deref() {
        Some("presentation-demo") => run_presentation_demo(),
        Some("approval-demo") => run_approval_control_demo(),
        _ => run_foundation_simulation(),
    }
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

fn run_approval_control_demo() {
    let mut runtime = FoundationRuntime::new(
        Default::default(),
        SequenceIdProvider::default(),
        SystemTimeProvider,
    );

    clear_demo_control_inbox();

    let proposal = ActionProposal::new(
        ActionId::new("approval-control-demo"),
        ActionKind::new("demo.safe_state_change"),
        Capability::new("demo.safe_state_change"),
        ActionImpact::StateChanging,
        ActionRisk::Moderate,
        "Apply the simulated state change",
        "approval-control-demo",
    );

    let outcome = runtime.evaluate_proposal(&proposal);
    assert!(matches!(
        outcome,
        MockExecutionOutcome::AwaitingUserApproval { .. }
    ));
    publish_snapshot(&runtime, "approval");
    println!(
        "Waiting for Approve/Reject from the Lychnos shell via {}",
        control_inbox_path().display()
    );

    for _ in 0..240 {
        if process_one_control_request(&mut runtime) {
            publish_snapshot(&runtime, "resolved");
            println!("Approval-control demo complete.");
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }

    println!("No shell decision received before demo timeout.");
}

fn process_one_control_request(runtime: &mut Runtime) -> bool {
    let inbox = control_inbox_path();
    let Ok(entries) = fs::read_dir(&inbox) else {
        return false;
    };

    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();

    for path in paths {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Ignoring unreadable control request {}: {error}",
                    path.display()
                );
                let _ = fs::remove_file(&path);
                continue;
            }
        };

        let envelope = match CompanionControlEnvelope::from_json(&contents) {
            Ok(envelope) => envelope,
            Err(error) => {
                eprintln!(
                    "Ignoring invalid control request {}: {error:?}",
                    path.display()
                );
                let _ = fs::remove_file(&path);
                continue;
            }
        };

        let request = envelope.request;
        let Some(proposal) = runtime.pending_action(&request.action_id).cloned() else {
            eprintln!(
                "Ignoring stale control request for non-pending action {}",
                request.action_id.as_str()
            );
            let _ = fs::remove_file(&path);
            continue;
        };

        if !request.matches_proposal(&proposal) {
            eprintln!(
                "Ignoring stale or mismatched control request for {}",
                request.action_id.as_str()
            );
            let _ = fs::remove_file(&path);
            continue;
        }

        let result = match request.decision {
            PendingApprovalDecision::Approve => runtime
                .approve_pending_action(&proposal, "desktop-shell-user")
                .map(|outcome| format!("approved -> {outcome:?}")),
            PendingApprovalDecision::Reject => runtime
                .reject_pending_action(&proposal, "desktop-shell-user")
                .map(|()| "rejected".to_string()),
        };

        match result {
            Ok(message) => println!(
                "Accepted shell decision for {}: {message}",
                request.action_id.as_str()
            ),
            Err(error) => eprintln!(
                "Runtime rejected shell decision for {}: {error:?}",
                request.action_id.as_str()
            ),
        }

        let _ = fs::remove_file(&path);
        return true;
    }

    false
}

fn control_inbox_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("lychnos/control-inbox-v1");
    }

    std::env::temp_dir().join("lychnos/control-inbox-v1")
}

fn clear_demo_control_inbox() {
    let inbox = control_inbox_path();
    let Ok(entries) = fs::read_dir(&inbox) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let _ = fs::remove_file(path);
        }
    }
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
