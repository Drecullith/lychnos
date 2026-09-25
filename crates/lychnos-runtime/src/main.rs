use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    interaction::{
        ConversationProvider, ConversationRequestEnvelope, ConversationResponseEnvelope,
        MockConversationProvider,
    },
    orchestrator::FoundationRuntime,
    persona::PersonaProfile,
    presentation::{
        CompanionControlEnvelope, CompanionPresentationEnvelope, PendingApprovalDecision,
    },
    providers::{SequenceIdProvider, SystemTimeProvider},
};

type Runtime = FoundationRuntime<SequenceIdProvider, SystemTimeProvider>;

fn main() {
    println!(
        "{} runtime v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    let mut runtime = FoundationRuntime::new(
        Default::default(),
        SequenceIdProvider::default(),
        SystemTimeProvider,
    );
    let persona = PersonaProfile::lychnos_default();
    let provider = MockConversationProvider;

    ensure_runtime_directories();
    publish_snapshot(&runtime);

    println!(
        "Runtime ready · persona={} · provider=local-mock",
        persona.display_name
    );

    loop {
        let runtime_changed = process_control_requests(&mut runtime);
        process_interaction_requests(&provider, &persona);

        if runtime_changed {
            publish_snapshot(&runtime);
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn process_control_requests(runtime: &mut Runtime) -> bool {
    let mut runtime_changed = false;

    for path in sorted_json_files(&control_inbox_path()) {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Ignoring unreadable control request {}: {error}",
                    path.display()
                );
                remove_request(&path);
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
                remove_request(&path);
                continue;
            }
        };

        let request = envelope.request;
        let Some(proposal) = runtime.pending_action(&request.action_id).cloned() else {
            eprintln!(
                "Ignoring stale control request for non-pending action {}",
                request.action_id.as_str()
            );
            remove_request(&path);
            continue;
        };

        if !request.matches_proposal(&proposal) {
            eprintln!(
                "Ignoring stale or mismatched control request for {}",
                request.action_id.as_str()
            );
            remove_request(&path);
            continue;
        }

        match request.decision {
            PendingApprovalDecision::Approve => {
                match runtime.approve_pending_action(&proposal, "desktop-shell-user") {
                    Ok(outcome) => {
                        println!(
                            "Accepted approval for {} -> {outcome:?}",
                            request.action_id.as_str()
                        );
                        runtime_changed = true;
                    }
                    Err(error) => eprintln!(
                        "Runtime rejected approval for {}: {error:?}",
                        request.action_id.as_str()
                    ),
                }
            }
            PendingApprovalDecision::Reject => {
                match runtime.reject_pending_action(&proposal, "desktop-shell-user") {
                    Ok(()) => {
                        println!("Accepted rejection for {}", request.action_id.as_str());
                        runtime_changed = true;
                    }
                    Err(error) => eprintln!(
                        "Runtime rejected rejection for {}: {error:?}",
                        request.action_id.as_str()
                    ),
                }
            }
        }

        remove_request(&path);
    }

    runtime_changed
}

fn process_interaction_requests(provider: &MockConversationProvider, persona: &PersonaProfile) {
    for path in sorted_json_files(&interaction_inbox_path()) {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Ignoring unreadable interaction request {}: {error}",
                    path.display()
                );
                remove_request(&path);
                continue;
            }
        };

        let envelope = match ConversationRequestEnvelope::from_json(&contents) {
            Ok(envelope) => envelope,
            Err(error) => {
                eprintln!(
                    "Ignoring invalid interaction request {}: {error:?}",
                    path.display()
                );
                remove_request(&path);
                continue;
            }
        };

        let request = envelope.request;
        if request.is_empty() {
            eprintln!("Ignoring empty interaction request {}", request.id.as_str());
            remove_request(&path);
            continue;
        }

        let response = match provider.respond(persona, &request) {
            Ok(response) => response,
            Err(never) => match never {},
        };

        if let Err(error) = publish_interaction_response(&response) {
            eprintln!(
                "Failed to publish response for {}: {error}",
                request.id.as_str()
            );
            continue;
        }

        println!(
            "Interaction {} handled from {:?}",
            request.id.as_str(),
            request.source
        );
        remove_request(&path);
    }
}

fn publish_snapshot(runtime: &Runtime) {
    let envelope = CompanionPresentationEnvelope::new(runtime.presentation_state());
    let json = envelope
        .to_json()
        .expect("presentation envelope should serialize");
    atomic_write(&presentation_snapshot_path(), &format!("{json}\n"))
        .expect("presentation snapshot should publish");
}

fn publish_interaction_response(
    response: &lychnos_core::interaction::ConversationResponse,
) -> Result<(), String> {
    let envelope = ConversationResponseEnvelope::new(response.clone());
    let json = envelope
        .to_json()
        .map_err(|error| format!("serialize response failed: {error}"))?;

    let target = interaction_outbox_path().join(format!(
        "response-{}-{}.json",
        std::process::id(),
        unique_nanos()
    ));
    atomic_write(&target, &format!("{json}\n"))
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create {} failed: {error}", parent.display()))?;

    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, contents)
        .map_err(|error| format!("write {} failed: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("publish {} failed: {error}", path.display()))
}

fn sorted_json_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
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
    paths
}

fn remove_request(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        eprintln!(
            "Failed to remove consumed request {}: {error}",
            path.display()
        );
    }
}

fn ensure_runtime_directories() {
    for path in [
        control_inbox_path(),
        interaction_inbox_path(),
        interaction_outbox_path(),
    ] {
        fs::create_dir_all(&path)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", path.display()));
    }
}

fn runtime_root() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("lychnos");
    }

    std::env::temp_dir().join("lychnos")
}

fn presentation_snapshot_path() -> PathBuf {
    runtime_root().join("presentation-v1.json")
}

fn control_inbox_path() -> PathBuf {
    runtime_root().join("control-inbox-v1")
}

fn interaction_inbox_path() -> PathBuf {
    runtime_root().join("interaction-inbox-v1")
}

fn interaction_outbox_path() -> PathBuf {
    runtime_root().join("interaction-outbox-v1")
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_nanos()
}
