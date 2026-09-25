mod audio;
mod stt;
mod tts;

use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    interaction::{
        ConversationProvider, ConversationRequest, ConversationRequestEnvelope,
        ConversationResponseEnvelope, InteractionId, InteractionSource, MockConversationProvider,
    },
    orchestrator::FoundationRuntime,
    persona::PersonaProfile,
    presentation::{
        CompanionControlEnvelope, CompanionPresentationEnvelope, PendingApprovalDecision,
    },
    providers::{SequenceIdProvider, SystemTimeProvider},
    speech::SpeechToTextProvider,
    voice::{
        VoiceCaptureCommand, VoiceCaptureControlEnvelope, VoiceCaptureState, VoiceCaptureStatus,
        VoiceCaptureStatusEnvelope,
    },
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
    let stt = match stt::WhisperCppStt::discover() {
        Ok(stt) => {
            println!("Speech-to-text ready · {}", stt.description());
            Some(stt)
        }
        Err(error) => {
            eprintln!("Speech-to-text unavailable: {error}");
            None
        }
    };
    let speech_output = match tts::PiperTts::discover() {
        Ok(tts) => {
            println!("Text-to-speech ready · {}", tts.description());
            Some(tts::SpeechOutputWorker::start(tts))
        }
        Err(error) => {
            eprintln!("Text-to-speech unavailable: {error}");
            None
        }
    };
    let mut active_capture: Option<audio::ActiveCapture> = None;

    report_audio_inputs();
    ensure_runtime_directories();
    clear_stale_voice_session_state();
    publish_snapshot(&runtime);

    println!(
        "Runtime ready · persona={} · provider=local-mock",
        persona.display_name
    );

    loop {
        let runtime_changed = process_control_requests(&mut runtime);
        process_voice_control_requests(
            &mut active_capture,
            stt.as_ref(),
            speech_output.as_ref(),
            &provider,
            &persona,
        );
        enforce_voice_capture_timeout(&mut active_capture);
        process_interaction_requests(&provider, &persona, speech_output.as_ref());

        if runtime_changed {
            publish_snapshot(&runtime);
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn report_audio_inputs() {
    match audio::discover_pipewire_inputs() {
        Ok(devices) if devices.is_empty() => {
            println!("Audio inputs: none detected");
        }
        Ok(devices) => {
            println!("Audio inputs detected: {}", devices.len());
            for device in devices {
                let default_marker = if device.is_default { " [default]" } else { "" };
                let channels = device
                    .channel_count
                    .map_or_else(|| "?ch".to_string(), |count| format!("{count}ch"));
                println!(
                    "  - {} · {} · {}{}",
                    device.display_name,
                    channels,
                    device.id.as_str(),
                    default_marker
                );
            }
        }
        Err(error) => {
            eprintln!("Audio input discovery unavailable: {error}");
        }
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

fn enforce_voice_capture_timeout(active_capture: &mut Option<audio::ActiveCapture>) {
    const MAX_PTT_DURATION: Duration = Duration::from_secs(60);

    if !active_capture
        .as_ref()
        .is_some_and(|capture| capture.elapsed() >= MAX_PTT_DURATION)
    {
        return;
    }

    let capture = active_capture
        .take()
        .expect("timed-out capture should still be active");
    let capture_id = capture.capture_id.as_str().to_string();

    match audio::stop_push_to_talk(capture) {
        Ok(completed) => {
            eprintln!(
                "PTT capture {} auto-stopped after 60s · {} bytes · {}",
                completed.capture_id.as_str(),
                completed.bytes,
                completed.path.display()
            );
            let _ = publish_voice_status(VoiceCaptureStatus {
                capture_id: completed.capture_id,
                state: VoiceCaptureState::TimedOut,
                captured_bytes: Some(completed.bytes),
                duration_ms: Some(completed.duration_ms),
                transcript: None,
                interaction_id: None,
                detail: "Push-to-talk reached the 60 second safety limit.".into(),
            });
        }
        Err(error) => {
            eprintln!("PTT capture {capture_id} timeout cleanup failed: {error}");
            let _ = publish_voice_status(VoiceCaptureStatus {
                capture_id: lychnos_core::voice::VoiceCaptureId::new(capture_id),
                state: VoiceCaptureState::Failed,
                captured_bytes: None,
                duration_ms: None,
                transcript: None,
                interaction_id: None,
                detail: format!("Timeout cleanup failed: {error}"),
            });
        }
    }
}

fn process_voice_control_requests(
    active_capture: &mut Option<audio::ActiveCapture>,
    stt: Option<&stt::WhisperCppStt>,
    speech_output: Option<&tts::SpeechOutputWorker>,
    provider: &MockConversationProvider,
    persona: &PersonaProfile,
) {
    for path in sorted_json_files(&voice_control_inbox_path()) {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!(
                    "Ignoring unreadable voice control {}: {error}",
                    path.display()
                );
                remove_request(&path);
                continue;
            }
        };

        let envelope = match VoiceCaptureControlEnvelope::from_json(&contents) {
            Ok(envelope) => envelope,
            Err(error) => {
                eprintln!(
                    "Ignoring invalid voice control {}: {error:?}",
                    path.display()
                );
                remove_request(&path);
                continue;
            }
        };

        let request = envelope.request;
        match request.command {
            VoiceCaptureCommand::StartPushToTalk => {
                if active_capture.is_some() {
                    eprintln!(
                        "Ignoring PTT start {} because capture is already active",
                        request.capture_id.as_str()
                    );
                } else {
                    match audio::start_push_to_talk(
                        request.capture_id.clone(),
                        request.device_id.as_ref(),
                    ) {
                        Ok(capture) => {
                            println!(
                                "PTT capture started · {} · {}",
                                capture.capture_id.as_str(),
                                capture.device.display_name
                            );
                            let _ = publish_voice_status(VoiceCaptureStatus {
                                capture_id: capture.capture_id.clone(),
                                state: VoiceCaptureState::Started,
                                captured_bytes: None,
                                duration_ms: None,
                                transcript: None,
                                interaction_id: None,
                                detail: format!("Listening on {}", capture.device.display_name),
                            });
                            *active_capture = Some(capture);
                        }
                        Err(error) => {
                            eprintln!(
                                "PTT capture {} failed to start: {error}",
                                request.capture_id.as_str()
                            );
                            let _ = publish_voice_status(VoiceCaptureStatus {
                                capture_id: request.capture_id.clone(),
                                state: VoiceCaptureState::Failed,
                                captured_bytes: None,
                                duration_ms: None,
                                transcript: None,
                                interaction_id: None,
                                detail: format!("Could not start microphone: {error}"),
                            });
                        }
                    }
                }
            }
            VoiceCaptureCommand::StopPushToTalk => {
                let Some(current) = active_capture.as_ref() else {
                    eprintln!(
                        "Ignoring PTT stop {} because no capture is active",
                        request.capture_id.as_str()
                    );
                    remove_request(&path);
                    continue;
                };

                if current.capture_id != request.capture_id {
                    eprintln!(
                        "Ignoring stale PTT stop {} while {} is active",
                        request.capture_id.as_str(),
                        current.capture_id.as_str()
                    );
                    remove_request(&path);
                    continue;
                }

                let capture = active_capture
                    .take()
                    .expect("active capture should still be present");
                match audio::stop_push_to_talk(capture) {
                    Ok(completed) => {
                        println!(
                            "PTT capture stopped · {} · {} bytes · {} · {}",
                            completed.capture_id.as_str(),
                            completed.bytes,
                            completed.device.display_name,
                            completed.path.display()
                        );

                        let capture_id = completed.capture_id.clone();
                        let captured_bytes = completed.bytes;
                        let duration_ms = completed.duration_ms;

                        let _ = publish_voice_status(VoiceCaptureStatus {
                            capture_id: capture_id.clone(),
                            state: VoiceCaptureState::Transcribing,
                            captured_bytes: Some(captured_bytes),
                            duration_ms: Some(duration_ms),
                            transcript: None,
                            interaction_id: None,
                            detail: "Transcribing locally with Whisper.".into(),
                        });

                        match handle_completed_voice_turn(
                            &completed,
                            stt,
                            speech_output,
                            provider,
                            persona,
                        ) {
                            Ok((transcript, interaction_id)) => {
                                println!(
                                    "PTT transcription · {} · {}",
                                    capture_id.as_str(),
                                    transcript
                                );
                                let _ = publish_voice_status(VoiceCaptureStatus {
                                    capture_id,
                                    state: VoiceCaptureState::Transcribed,
                                    captured_bytes: Some(captured_bytes),
                                    duration_ms: Some(duration_ms),
                                    transcript: Some(transcript),
                                    interaction_id: Some(interaction_id),
                                    detail: "Local transcription complete.".into(),
                                });
                            }
                            Err(error) => {
                                eprintln!(
                                    "PTT transcription {} failed: {error}",
                                    capture_id.as_str()
                                );
                                let _ = publish_voice_status(VoiceCaptureStatus {
                                    capture_id,
                                    state: VoiceCaptureState::Failed,
                                    captured_bytes: Some(captured_bytes),
                                    duration_ms: Some(duration_ms),
                                    transcript: None,
                                    interaction_id: None,
                                    detail: format!("Speech recognition failed: {error}"),
                                });
                            }
                        }

                        if let Err(error) = fs::remove_file(&completed.path) {
                            eprintln!(
                                "Failed to delete ephemeral voice capture {}: {error}",
                                completed.path.display()
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "PTT capture {} failed to stop cleanly: {error}",
                            request.capture_id.as_str()
                        );
                        let _ = publish_voice_status(VoiceCaptureStatus {
                            capture_id: request.capture_id.clone(),
                            state: VoiceCaptureState::Failed,
                            captured_bytes: None,
                            duration_ms: None,
                            transcript: None,
                            interaction_id: None,
                            detail: format!("Could not finalize microphone capture: {error}"),
                        });
                    }
                }
            }
        }

        remove_request(&path);
    }
}

fn handle_completed_voice_turn(
    completed: &audio::CompletedCapture,
    stt: Option<&stt::WhisperCppStt>,
    speech_output: Option<&tts::SpeechOutputWorker>,
    provider: &MockConversationProvider,
    persona: &PersonaProfile,
) -> Result<(String, InteractionId), String> {
    let stt = stt.ok_or_else(|| {
        "local STT is not installed; run scripts/install-local-stt.sh".to_string()
    })?;

    let transcript = stt.transcribe(&completed.path)?;
    let text = transcript.text.trim().to_string();
    if text.is_empty() {
        return Err("Whisper returned no speech".into());
    }

    let interaction_id = InteractionId::new(format!("ptt-{}", completed.capture_id.as_str()));
    let request = ConversationRequest::new(
        interaction_id.clone(),
        InteractionSource::PushToTalk,
        text.clone(),
    );

    handle_conversation_request(provider, persona, speech_output, &request)?;

    Ok((text, interaction_id))
}

fn handle_conversation_request(
    provider: &MockConversationProvider,
    persona: &PersonaProfile,
    speech_output: Option<&tts::SpeechOutputWorker>,
    request: &ConversationRequest,
) -> Result<(), String> {
    let response = match provider.respond(persona, request) {
        Ok(response) => response,
        Err(never) => match never {},
    };

    publish_interaction_response(&response)?;

    if request.source != InteractionSource::Typed
        && let Some(speech_output) = speech_output
        && let Err(error) = speech_output.speak(response.text.clone())
    {
        eprintln!("Failed to queue spoken Lychnos reply: {error}");
    }

    Ok(())
}

fn process_interaction_requests(
    provider: &MockConversationProvider,
    persona: &PersonaProfile,
    speech_output: Option<&tts::SpeechOutputWorker>,
) {
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

        if let Err(error) = handle_conversation_request(provider, persona, speech_output, &request)
        {
            eprintln!(
                "Failed to handle interaction {}: {error}",
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

fn publish_voice_status(status: VoiceCaptureStatus) -> Result<(), String> {
    let envelope = VoiceCaptureStatusEnvelope::new(status);
    let json = envelope
        .to_json()
        .map_err(|error| format!("serialize voice status failed: {error}"))?;
    atomic_write(&voice_status_path(), &format!("{json}\n"))
}

fn clear_stale_voice_session_state() {
    let inbox = voice_control_inbox_path();
    if let Ok(entries) = fs::read_dir(&inbox) {
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
    let _ = fs::remove_file(voice_status_path());
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
        voice_control_inbox_path(),
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

fn voice_control_inbox_path() -> PathBuf {
    runtime_root().join("voice-control-inbox-v1")
}

fn voice_status_path() -> PathBuf {
    runtime_root().join("voice-status-v1.json")
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
