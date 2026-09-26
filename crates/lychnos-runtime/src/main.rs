mod audio;
mod local_brain;
mod memory_context;
mod memory_store;
mod perception_v2;
mod stt;
mod system_health;
mod tts;
mod wake_word;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    initiative::{
        InitiativeContext, InitiativeDecision, InitiativeMode, InitiativeObservation,
        InitiativePolicy, InitiativeProvider, InitiativeTrigger,
    },
    interaction::{
        ConversationOutputMode, ConversationProvider, ConversationRequest,
        ConversationRequestEnvelope, ConversationResponse, ConversationResponseEnvelope,
        InteractionId, InteractionSource,
    },
    orchestrator::FoundationRuntime,
    perception::{PerceptionControl, PerceptionEvent, PerceptionUtteranceSource},
    persona::PersonaProfile,
    presentation::{
        CompanionActivity, CompanionControlEnvelope, CompanionPresentationEnvelope,
        PendingApprovalDecision,
    },
    providers::{SequenceIdProvider, SystemTimeProvider},
    speech::SpeechToTextProvider,
    voice::{
        VoiceCaptureCommand, VoiceCaptureControlEnvelope, VoiceCaptureState, VoiceCaptureStatus,
        VoiceCaptureStatusEnvelope,
    },
};

type Runtime = FoundationRuntime<SequenceIdProvider, SystemTimeProvider>;

struct ConversationRuntimeContext<'a> {
    runtime: &'a Runtime,
    activity: &'a mut CompanionActivity,
    provider_label: &'a str,
    provider: &'a local_brain::RuntimeBrain,
    persona: &'a PersonaProfile,
    speech_output: Option<&'a tts::SpeechOutputWorker>,
    memory_store: &'a mut Option<memory_store::JsonFileMemoryStore>,
}

impl ConversationRuntimeContext<'_> {
    fn set_activity(&mut self, next: CompanionActivity) {
        set_activity(self.runtime, self.activity, next, self.provider_label);
    }
}

const INITIATIVE_IDLE_BEFORE_CHECK: Duration = Duration::from_secs(60);

struct InitiativeScheduler {
    started_at: Instant,
    last_user_activity: Option<Instant>,
    last_surface: Option<Instant>,
    context_revision: u64,
    considered_context_revision: u64,
    pending_trigger: Option<InitiativeTrigger>,
    pending_observations: Vec<InitiativeObservation>,
    mode: InitiativeMode,
    policy: InitiativePolicy,
}

impl InitiativeScheduler {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            last_user_activity: None,
            last_surface: None,
            context_revision: 0,
            considered_context_revision: 0,
            pending_trigger: None,
            pending_observations: Vec::new(),
            mode: InitiativeMode::Normal,
            policy: InitiativePolicy::default(),
        }
    }

    fn note_user_activity(&mut self) {
        self.last_user_activity = Some(Instant::now());
    }

    fn note_context_change(&mut self, trigger: InitiativeTrigger) {
        self.context_revision = self.context_revision.wrapping_add(1).max(1);
        self.pending_trigger = Some(trigger);
        self.pending_observations.clear();
    }

    fn note_observation(&mut self, trigger: InitiativeTrigger, observation: InitiativeObservation) {
        self.context_revision = self.context_revision.wrapping_add(1).max(1);

        if self.pending_trigger != Some(trigger) {
            self.pending_observations.clear();
        }

        self.pending_trigger = Some(trigger);
        if self.pending_observations.len() < 8 {
            self.pending_observations.push(observation);
        }
    }

    fn context_if_due(
        &self,
        runtime: &Runtime,
        has_session_context: bool,
    ) -> Option<InitiativeContext> {
        if (!has_session_context && self.pending_observations.is_empty())
            || self.mode == InitiativeMode::Off
            || self.context_revision == self.considered_context_revision
        {
            return None;
        }

        let trigger = self.pending_trigger?;
        let now = Instant::now();
        let last_activity = self.last_user_activity.unwrap_or(self.started_at);
        let user_idle = now.saturating_duration_since(last_activity);

        if user_idle < INITIATIVE_IDLE_BEFORE_CHECK {
            return None;
        }

        let presentation = runtime.presentation_state();
        if presentation.runtime_mode != lychnos_core::runtime::RuntimeMode::Normal
            || !presentation.pending_approvals.is_empty()
        {
            return None;
        }

        Some(InitiativeContext {
            trigger,
            observations: self.pending_observations.clone(),
            runtime_mode: presentation.runtime_mode,
            initiative_mode: self.mode,
            milliseconds_since_user_interaction: duration_millis_u64(user_idle),
            milliseconds_since_last_surface: self
                .last_surface
                .map(|last| duration_millis_u64(now.saturating_duration_since(last))),
            user_is_interacting: false,
            has_pending_approval: false,
        })
    }

    fn mark_checked(&mut self) {
        self.considered_context_revision = self.context_revision;
        self.pending_trigger = None;
        self.pending_observations.clear();
    }

    fn mark_surface(&mut self) {
        self.last_surface = Some(Instant::now());
    }
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn main() {
    println!(
        "{} runtime v{}",
        lychnos_core::PROJECT_NAME,
        lychnos_core::version()
    );

    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let signal_shutdown = Arc::clone(&shutdown_requested);
    if let Err(error) = ctrlc::set_handler(move || {
        signal_shutdown.store(true, Ordering::Release);
    }) {
        eprintln!("Graceful shutdown handler unavailable: {error}");
    }

    let mut runtime = FoundationRuntime::new(
        Default::default(),
        SequenceIdProvider::default(),
        SystemTimeProvider,
    );
    let persona = PersonaProfile::lychnos_default();
    let provider = local_brain::RuntimeBrain::discover();
    let provider_label = provider.description();
    let mut activity = CompanionActivity::Idle;
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
    let mut memory_store = match memory_store::JsonFileMemoryStore::open_default() {
        Ok(store) => {
            println!(
                "Memory ready · local JSON v1 · {} records · {}",
                store.len(),
                store.path().display()
            );
            Some(store)
        }
        Err(error) => {
            eprintln!("Persistent memory unavailable: {error}");
            None
        }
    };
    let mut active_capture: Option<audio::ActiveCapture> = None;
    let mut initiative_scheduler = InitiativeScheduler::new();
    let mut system_health_collector = system_health::UserServiceHealthCollector::new();
    let mut hands_free_session = false;
    let mut voice_v2_session = false;
    let mut voice_v2_follow_up_after_reply = true;
    let mut perception_suspended = false;

    report_audio_inputs();
    ensure_runtime_directories();
    clear_stale_voice_session_state();

    let mut perception_v2 = match audio::prepare_default_input() {
        Ok(device) => match perception_v2::PerceptionWorker::start(&device) {
            Ok(worker) => {
                println!(
                    "Voice V2 ready · dedicated KWS + WebRTC VAD · phrase=Lychnos · {}",
                    device.display_name
                );
                Some(worker)
            }
            Err(error) => {
                eprintln!("Voice V2 unavailable: {error}");
                None
            }
        },
        Err(error) => {
            eprintln!("Voice V2 unavailable: {error}");
            None
        }
    };

    let mut wake_word = if perception_v2.is_some() {
        println!("Voice V1 wake listener disabled · Voice V2 owns ambient microphone");
        None
    } else {
        match stt.as_ref() {
            Some(stt) => match audio::prepare_default_input() {
                Ok(device) => match wake_word::WakeWordWorker::start(stt.clone(), device.clone()) {
                    Ok(worker) => {
                        println!(
                            "Wake word ready · local Whisper-gated listener · phrase=Lychnos · {}",
                            device.display_name
                        );
                        Some(worker)
                    }
                    Err(error) => {
                        eprintln!("Wake word unavailable: {error}");
                        None
                    }
                },
                Err(error) => {
                    eprintln!("Wake word unavailable: {error}");
                    None
                }
            },
            None => {
                eprintln!("Wake word unavailable: local STT is not ready");
                None
            }
        }
    };

    publish_snapshot(&runtime, activity, &provider_label);

    println!(
        "Runtime ready · persona={} · provider={}",
        persona.display_name, provider_label
    );

    while !shutdown_requested.load(Ordering::Acquire) {
        let runtime_changed = process_control_requests(&mut runtime);
        let ambient_changed = process_system_health(
            &mut runtime,
            &mut system_health_collector,
            &mut initiative_scheduler,
        );

        let wake_suspended = runtime.presentation_state().runtime_mode
            != lychnos_core::runtime::RuntimeMode::Normal
            || active_capture.is_some()
            || matches!(
                activity,
                CompanionActivity::Thinking | CompanionActivity::Speaking
            );

        if let Some(worker) = wake_word.as_ref() {
            worker.set_suspended(wake_suspended);
        }

        let should_suspend_perception = runtime.presentation_state().runtime_mode
            != lychnos_core::runtime::RuntimeMode::Normal
            || active_capture.is_some();
        if should_suspend_perception != perception_suspended {
            if let Some(worker) = perception_v2.as_mut() {
                let control = if should_suspend_perception {
                    PerceptionControl::Disable
                } else {
                    PerceptionControl::Enable
                };
                if let Err(error) = worker.send(control) {
                    eprintln!("Voice V2 control failed: {error}");
                }
            }
            perception_suspended = should_suspend_perception;
        }

        {
            let mut conversation = ConversationRuntimeContext {
                runtime: &runtime,
                activity: &mut activity,
                provider_label: &provider_label,
                provider: &provider,
                persona: &persona,
                speech_output: speech_output.as_ref(),
                memory_store: &mut memory_store,
            };

            process_perception_v2_events(
                &mut conversation,
                &mut perception_v2,
                stt.as_ref(),
                &mut voice_v2_session,
                &mut voice_v2_follow_up_after_reply,
                &mut initiative_scheduler,
            );
            process_wake_word_events(
                &mut conversation,
                &mut wake_word,
                &mut hands_free_session,
                &mut initiative_scheduler,
            );
            process_voice_control_requests(
                &mut conversation,
                &mut active_capture,
                stt.as_ref(),
                &mut initiative_scheduler,
            );
            process_interaction_requests(&mut conversation, &mut initiative_scheduler);
        }

        enforce_voice_capture_timeout(
            &runtime,
            &mut activity,
            &provider_label,
            &mut active_capture,
        );
        process_initiative(
            &runtime,
            &mut activity,
            &provider_label,
            &provider,
            &persona,
            speech_output.as_ref(),
            &mut initiative_scheduler,
        );

        if activity == CompanionActivity::Speaking
            && speech_output
                .as_ref()
                .is_none_or(|worker| !worker.is_speaking())
        {
            if voice_v2_session {
                if let Some(worker) = perception_v2.as_mut() {
                    let follow_up = voice_v2_follow_up_after_reply;
                    match worker.send(PerceptionControl::ResponseFinished { follow_up }) {
                        Ok(()) if follow_up => {
                            activity = CompanionActivity::Listening;
                        }
                        Ok(()) => {
                            voice_v2_session = false;
                            activity = CompanionActivity::Idle;
                        }
                        Err(error) => {
                            eprintln!("Voice V2 reply-complete control failed: {error}");
                            voice_v2_session = false;
                            activity = CompanionActivity::Idle;
                        }
                    }
                } else {
                    voice_v2_session = false;
                    activity = CompanionActivity::Idle;
                }
            } else if hands_free_session {
                if let Some(worker) = wake_word.as_ref() {
                    worker.open_follow_up_window();
                    println!("Hands-free follow-up window opened");
                    activity = CompanionActivity::Listening;
                } else {
                    hands_free_session = false;
                    activity = CompanionActivity::Idle;
                }
            } else {
                activity = CompanionActivity::Idle;
            }
            publish_snapshot(&runtime, activity, &provider_label);
        } else if runtime_changed || ambient_changed {
            publish_snapshot(&runtime, activity, &provider_label);
        }

        thread::sleep(Duration::from_millis(100));
    }

    println!("Lychnos runtime shutting down cleanly");
}

fn process_system_health(
    runtime: &mut Runtime,
    collector: &mut system_health::UserServiceHealthCollector,
    scheduler: &mut InitiativeScheduler,
) -> bool {
    match runtime.collect_once(collector) {
        Ok(Some(cycle)) => {
            let observation = system_health::observation_for_event(&cycle.event);
            println!(
                "Ambient event · {} · {}",
                cycle.event.kind.as_str(),
                observation.summary
            );
            scheduler.note_observation(InitiativeTrigger::DiagnosticChange, observation);
            true
        }
        Ok(None) => false,
        Err(error) => {
            eprintln!("System-health collector failed: {error:?}");
            true
        }
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

fn enforce_voice_capture_timeout(
    runtime: &Runtime,
    activity: &mut CompanionActivity,
    provider_label: &str,
    active_capture: &mut Option<audio::ActiveCapture>,
) {
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

    set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
}

fn process_perception_v2_events(
    context: &mut ConversationRuntimeContext<'_>,
    perception: &mut Option<perception_v2::PerceptionWorker>,
    stt: Option<&stt::WhisperCppStt>,
    voice_v2_session: &mut bool,
    follow_up_after_reply: &mut bool,
    initiative_scheduler: &mut InitiativeScheduler,
) {
    loop {
        let event = perception
            .as_ref()
            .and_then(perception_v2::PerceptionWorker::try_recv);
        let Some(event) = event else {
            break;
        };

        match event {
            PerceptionEvent::Ready {
                wake_phrase,
                device,
            } => {
                println!("Voice V2 perception armed · phrase={wake_phrase} · {device}");
            }
            PerceptionEvent::State { state } => {
                println!("Voice V2 state · {state:?}");
            }
            PerceptionEvent::WakeDetected { keyword } => {
                *voice_v2_session = true;
                *follow_up_after_reply = true;
                initiative_scheduler.note_user_activity();
                context.set_activity(CompanionActivity::Listening);
                println!("Voice V2 wake detected · {keyword}");

                if let Err(error) = wake_word::play_acknowledgement() {
                    eprintln!("Voice V2 acknowledgement unavailable: {error}");
                }
            }
            PerceptionEvent::UtteranceFinalized {
                path,
                duration_ms,
                source,
            } => {
                *voice_v2_session = true;
                initiative_scheduler.note_user_activity();
                context.set_activity(CompanionActivity::Thinking);

                let capture_id = lychnos_core::voice::VoiceCaptureId::new(format!(
                    "voice-v2-{}",
                    unique_nanos()
                ));
                let audio_path = PathBuf::from(&path);

                let transcript = match stt {
                    Some(stt) => match stt.transcribe(&audio_path) {
                        Ok(transcript) => transcript.text,
                        Err(error) => {
                            eprintln!("Voice V2 transcription failed: {error}");
                            let _ = fs::remove_file(&audio_path);
                            reset_perception_after_failed_turn(
                                perception,
                                voice_v2_session,
                                context,
                            );
                            continue;
                        }
                    },
                    None => {
                        eprintln!("Voice V2 transcription unavailable: local STT is not ready");
                        let _ = fs::remove_file(&audio_path);
                        reset_perception_after_failed_turn(perception, voice_v2_session, context);
                        continue;
                    }
                };
                let _ = fs::remove_file(&audio_path);

                let command = match source {
                    PerceptionUtteranceSource::Wake => strip_voice_v2_wake_prefix(&transcript),
                    PerceptionUtteranceSource::FollowUp => transcript.trim().to_string(),
                };

                println!(
                    "Voice V2 transcription · {} · {}ms · {:?} · command={} · raw={}",
                    capture_id.as_str(),
                    duration_ms,
                    source,
                    command,
                    transcript
                );

                if command.trim().is_empty() {
                    if let Some(worker) = perception.as_mut() {
                        let _ = worker.send(PerceptionControl::ResponseStarted);
                        if let Err(error) =
                            worker.send(PerceptionControl::ResponseFinished { follow_up: true })
                        {
                            eprintln!("Voice V2 wake-only follow-up failed: {error}");
                            *voice_v2_session = false;
                            context.set_activity(CompanionActivity::Idle);
                            continue;
                        }
                    }
                    *follow_up_after_reply = true;
                    context.set_activity(CompanionActivity::Listening);
                    continue;
                }

                let close_after_reply = wake_word::should_close_session(&command);
                *follow_up_after_reply = !close_after_reply;

                let interaction_source = match source {
                    PerceptionUtteranceSource::Wake => InteractionSource::WakeWord,
                    PerceptionUtteranceSource::FollowUp => InteractionSource::VoiceSession,
                };
                let interaction_id =
                    InteractionId::new(format!("voice-v2-{}", capture_id.as_str()));
                let request = ConversationRequest::new(
                    interaction_id.clone(),
                    interaction_source,
                    command.clone(),
                );

                let _ = publish_voice_status(VoiceCaptureStatus {
                    capture_id: capture_id.clone(),
                    state: VoiceCaptureState::Transcribed,
                    captured_bytes: None,
                    duration_ms: Some(duration_ms),
                    transcript: Some(command.clone()),
                    interaction_id: Some(interaction_id.clone()),
                    detail: "Voice V2 speech recognized locally.".into(),
                });

                match handle_conversation_request(context, &request) {
                    Ok(()) => {
                        initiative_scheduler
                            .note_context_change(InitiativeTrigger::ConversationFollowUp);

                        if let Some(worker) = perception.as_mut() {
                            if let Err(error) = worker.send(PerceptionControl::ResponseStarted) {
                                eprintln!("Voice V2 response-start control failed: {error}");
                                *voice_v2_session = false;
                                context.set_activity(CompanionActivity::Idle);
                                continue;
                            }

                            if *context.activity != CompanionActivity::Speaking {
                                match worker.send(PerceptionControl::ResponseFinished {
                                    follow_up: !close_after_reply,
                                }) {
                                    Ok(()) if !close_after_reply => {
                                        context.set_activity(CompanionActivity::Listening);
                                    }
                                    Ok(()) => {
                                        *voice_v2_session = false;
                                        context.set_activity(CompanionActivity::Idle);
                                    }
                                    Err(error) => {
                                        eprintln!(
                                            "Voice V2 immediate reply-complete control failed: {error}"
                                        );
                                        *voice_v2_session = false;
                                        context.set_activity(CompanionActivity::Idle);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "Voice V2 interaction {} failed: {error}",
                            interaction_id.as_str()
                        );
                        reset_perception_after_failed_turn(perception, voice_v2_session, context);
                    }
                }
            }
            PerceptionEvent::FollowUpSpeakerRejected { score_milli } => {
                println!(
                    "Voice V2 ignored non-session speaker · similarity={:.3}",
                    f32::from(score_milli) / 1000.0
                );
            }
            PerceptionEvent::FollowUpExpired => {
                if *voice_v2_session {
                    println!("Voice V2 follow-up window expired");
                }
                *voice_v2_session = false;
                if *context.activity == CompanionActivity::Listening {
                    context.set_activity(CompanionActivity::Idle);
                }
            }
        }
    }

    let stopped = perception
        .as_mut()
        .is_some_and(|worker| !worker.is_running());
    if stopped {
        eprintln!("Voice V2 perception process stopped");
        *perception = None;
        *voice_v2_session = false;
        if *context.activity == CompanionActivity::Listening {
            context.set_activity(CompanionActivity::Idle);
        }
    }
}

fn reset_perception_after_failed_turn(
    perception: &mut Option<perception_v2::PerceptionWorker>,
    voice_v2_session: &mut bool,
    context: &mut ConversationRuntimeContext<'_>,
) {
    if let Some(worker) = perception.as_mut() {
        let _ = worker.send(PerceptionControl::ResponseStarted);
        let _ = worker.send(PerceptionControl::ResponseFinished { follow_up: false });
    }
    *voice_v2_session = false;
    context.set_activity(CompanionActivity::Idle);
}

fn strip_voice_v2_wake_prefix(transcript: &str) -> String {
    let trimmed = transcript.trim();
    let lower = trimmed.to_ascii_lowercase();
    let aliases = [
        "lychnos",
        "lichnos",
        "lichlaus",
        "lich nos",
        "lich noss",
        "leek nos",
        "leek noss",
        "lick nos",
        "lick noss",
        "lick nuss",
        "lee h nos",
        "lee h noss",
    ];

    for alias in aliases {
        if lower.starts_with(alias) {
            let remainder = &trimmed[alias.len()..];
            return remainder
                .trim_start_matches(|character: char| {
                    character.is_whitespace()
                        || matches!(character, ',' | '.' | ':' | ';' | '!' | '?' | '-')
                })
                .trim()
                .to_string();
        }
    }

    if let Some(comma) = trimmed.find(',')
        && comma <= 14
    {
        return trimmed[comma + 1..].trim().to_string();
    }

    trimmed.to_string()
}

fn process_wake_word_events(
    context: &mut ConversationRuntimeContext<'_>,
    wake_word: &mut Option<wake_word::WakeWordWorker>,
    hands_free_session: &mut bool,
    initiative_scheduler: &mut InitiativeScheduler,
) {
    let mut disable_worker = false;

    loop {
        let event = wake_word
            .as_ref()
            .and_then(wake_word::WakeWordWorker::try_recv);
        let Some(event) = event else {
            break;
        };

        match event {
            wake_word::WakeWordEvent::Activated {
                capture_id,
                transcript,
            } => {
                *hands_free_session = true;
                initiative_scheduler.note_user_activity();
                println!("Wake word detected · {transcript}");
                context.set_activity(CompanionActivity::Listening);

                if let Some(worker) = wake_word.as_ref() {
                    worker.set_suspended(true);
                }
                if let Err(error) = wake_word::play_acknowledgement() {
                    eprintln!("Wake acknowledgement unavailable: {error}");
                }
                if let Some(worker) = wake_word.as_ref() {
                    worker.set_suspended(false);
                }

                let _ = publish_voice_status(VoiceCaptureStatus {
                    capture_id,
                    state: VoiceCaptureState::Started,
                    captured_bytes: None,
                    duration_ms: None,
                    transcript: None,
                    interaction_id: None,
                    detail: "Wake word detected. Listening for hands-free speech.".into(),
                });
            }
            wake_word::WakeWordEvent::Command {
                capture_id,
                source,
                transcript,
                command,
            } => {
                let close_after_reply = wake_word::should_close_session(&command);
                *hands_free_session = !close_after_reply;
                initiative_scheduler.note_user_activity();

                if close_after_reply {
                    if let Some(worker) = wake_word.as_ref() {
                        worker.clear_follow_up_window();
                    }
                    println!("Hands-free closing phrase detected");
                }

                let prefix = match source {
                    InteractionSource::WakeWord => "wake",
                    InteractionSource::VoiceSession => "voice-session",
                    _ => "voice",
                };
                let interaction_id =
                    InteractionId::new(format!("{prefix}-{}", capture_id.as_str()));
                let request =
                    ConversationRequest::new(interaction_id.clone(), source, command.clone());

                let _ = publish_voice_status(VoiceCaptureStatus {
                    capture_id: capture_id.clone(),
                    state: VoiceCaptureState::Transcribed,
                    captured_bytes: None,
                    duration_ms: None,
                    transcript: Some(command.clone()),
                    interaction_id: Some(interaction_id.clone()),
                    detail: "Hands-free speech recognized locally.".into(),
                });

                println!(
                    "Hands-free transcription · {} · {} · raw={}",
                    capture_id.as_str(),
                    command,
                    transcript
                );

                match handle_conversation_request(context, &request) {
                    Ok(()) => {
                        initiative_scheduler
                            .note_context_change(InitiativeTrigger::ConversationFollowUp);

                        if *context.activity == CompanionActivity::Idle {
                            if let Some(worker) = wake_word.as_ref() {
                                worker.open_follow_up_window();
                                println!("Hands-free follow-up window opened");
                                context.set_activity(CompanionActivity::Listening);
                            } else {
                                *hands_free_session = false;
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "Hands-free interaction {} failed: {error}",
                            interaction_id.as_str()
                        );
                        *hands_free_session = false;
                        if let Some(worker) = wake_word.as_ref() {
                            worker.clear_follow_up_window();
                        }
                        context.set_activity(CompanionActivity::Idle);
                    }
                }
            }
            wake_word::WakeWordEvent::SessionExpired => {
                if *hands_free_session {
                    println!("Hands-free voice session expired");
                }
                *hands_free_session = false;
                if *context.activity == CompanionActivity::Listening {
                    context.set_activity(CompanionActivity::Idle);
                }
            }
            wake_word::WakeWordEvent::Error(error) => {
                eprintln!("Wake-word listener stopped: {error}");
                *hands_free_session = false;
                context.set_activity(CompanionActivity::Idle);
                disable_worker = true;
            }
        }
    }

    if disable_worker {
        *wake_word = None;
    }
}

fn process_voice_control_requests(
    context: &mut ConversationRuntimeContext<'_>,
    active_capture: &mut Option<audio::ActiveCapture>,
    stt: Option<&stt::WhisperCppStt>,
    initiative_scheduler: &mut InitiativeScheduler,
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
                initiative_scheduler.note_user_activity();
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
                            context.set_activity(CompanionActivity::Listening);
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
                            context.set_activity(CompanionActivity::Idle);
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
                        context.set_activity(CompanionActivity::Thinking);

                        match handle_completed_voice_turn(context, &completed, stt) {
                            Ok((transcript, interaction_id)) => {
                                initiative_scheduler
                                    .note_context_change(InitiativeTrigger::ConversationFollowUp);
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
                                context.set_activity(CompanionActivity::Idle);
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
                        context.set_activity(CompanionActivity::Idle);
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
    context: &mut ConversationRuntimeContext<'_>,
    completed: &audio::CompletedCapture,
    stt: Option<&stt::WhisperCppStt>,
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

    handle_conversation_request(context, &request)?;

    Ok((text, interaction_id))
}

fn handle_conversation_request(
    context: &mut ConversationRuntimeContext<'_>,
    request: &ConversationRequest,
) -> Result<(), String> {
    let assembled =
        memory_context::prepare_conversation_context(context.memory_store.as_mut(), request)?;

    for notice in &assembled.runtime_notices {
        println!("Conversation context · {notice}");
    }

    context.set_activity(CompanionActivity::Thinking);

    let response = match context
        .provider
        .respond(context.persona, &assembled, request)
    {
        Ok(response) => response,
        Err(error) => {
            context.set_activity(CompanionActivity::Idle);
            return Err(error);
        }
    };

    println!(
        "Conversation response ready · {} · source={:?} · output={:?} · {} chars",
        request.id.as_str(),
        request.source,
        request.output_mode,
        response.text.chars().count()
    );

    if let Err(error) = publish_interaction_response(&response) {
        context.set_activity(CompanionActivity::Idle);
        return Err(error);
    }

    if request.output_mode == ConversationOutputMode::TextAndSpeech {
        if let Some(speech_output) = context.speech_output {
            match speech_output.speak(response.text.clone()) {
                Ok(()) => {
                    println!("Speech reply queued · {}", request.id.as_str());
                    context.set_activity(CompanionActivity::Speaking);
                }
                Err(error) => {
                    eprintln!("Failed to queue spoken Lychnos reply: {error}");
                    context.set_activity(CompanionActivity::Idle);
                }
            }
        } else {
            context.set_activity(CompanionActivity::Idle);
        }
    } else {
        context.set_activity(CompanionActivity::Idle);
    }

    Ok(())
}

fn process_interaction_requests(
    context: &mut ConversationRuntimeContext<'_>,
    initiative_scheduler: &mut InitiativeScheduler,
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

        initiative_scheduler.note_user_activity();

        if let Err(error) = handle_conversation_request(context, &request) {
            eprintln!(
                "Failed to handle interaction {}: {error}",
                request.id.as_str()
            );
            continue;
        }

        initiative_scheduler.note_context_change(InitiativeTrigger::ConversationFollowUp);

        println!(
            "Interaction {} handled from {:?}",
            request.id.as_str(),
            request.source
        );
        remove_request(&path);
    }
}

fn process_initiative(
    runtime: &Runtime,
    activity: &mut CompanionActivity,
    provider_label: &str,
    provider: &local_brain::RuntimeBrain,
    persona: &PersonaProfile,
    speech_output: Option<&tts::SpeechOutputWorker>,
    scheduler: &mut InitiativeScheduler,
) {
    // Initiative must never steal lifecycle ownership from an active
    // conversation. In particular, changing Speaking -> Idle here can cause
    // Voice V2 to miss speech-completion and never re-arm the follow-up window.
    if *activity != CompanionActivity::Idle {
        return;
    }

    let Some(context) = scheduler.context_if_due(runtime, provider.has_session_context()) else {
        return;
    };

    scheduler.mark_checked();
    set_activity(
        runtime,
        activity,
        CompanionActivity::Thinking,
        provider_label,
    );

    let candidate = match provider.propose(persona, &context) {
        Ok(Some(candidate)) => candidate,
        Ok(None) => {
            set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
            return;
        }
        Err(error) => {
            eprintln!("Initiative proposal failed: {error}");
            set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
            return;
        }
    };

    match scheduler.policy.evaluate(&context, &candidate) {
        InitiativeDecision::Surface => {
            let interaction_id = InteractionId::new(format!(
                "initiative-{}-{}",
                std::process::id(),
                unique_nanos()
            ));
            let response =
                ConversationResponse::new(interaction_id, persona, candidate.message.clone());

            if let Err(error) = publish_interaction_response(&response) {
                eprintln!("Failed to publish initiative response: {error}");
                set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
                return;
            }

            if let Some(speech_output) = speech_output {
                match speech_output.speak(candidate.message.clone()) {
                    Ok(()) => set_activity(
                        runtime,
                        activity,
                        CompanionActivity::Speaking,
                        provider_label,
                    ),
                    Err(error) => {
                        eprintln!("Failed to queue proactive Lychnos speech: {error}");
                        set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
                    }
                }
            } else {
                set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
            }

            if let Err(error) = provider.record_proactive_surface(&candidate.message) {
                eprintln!("Failed to record proactive surface in session context: {error}");
            }

            scheduler.mark_surface();
            println!(
                "Initiative surfaced · {:?} · {}",
                candidate.trigger, candidate.reason_summary
            );
        }
        InitiativeDecision::Suppress(reason) => {
            set_activity(runtime, activity, CompanionActivity::Idle, provider_label);
            println!("Initiative suppressed · {reason:?}");
        }
    }
}

fn set_activity(
    runtime: &Runtime,
    activity: &mut CompanionActivity,
    next: CompanionActivity,
    provider_label: &str,
) {
    if *activity == next {
        return;
    }

    *activity = next;
    publish_snapshot(runtime, next, provider_label);
}

fn publish_snapshot(runtime: &Runtime, activity: CompanionActivity, provider_label: &str) {
    let mut state = runtime.presentation_state();
    state.activity = activity;
    state.intelligence_label = provider_label.to_string();
    let envelope = CompanionPresentationEnvelope::new(state);
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

#[cfg(test)]
mod scheduler_tests {
    use super::*;

    fn runtime() -> Runtime {
        FoundationRuntime::new(
            Default::default(),
            SequenceIdProvider::default(),
            SystemTimeProvider,
        )
    }

    fn idle_scheduler_with_trigger(trigger: InitiativeTrigger) -> InitiativeScheduler {
        let mut scheduler = InitiativeScheduler::new();
        scheduler.started_at =
            Instant::now() - INITIATIVE_IDLE_BEFORE_CHECK - Duration::from_secs(1);
        scheduler.note_context_change(trigger);
        scheduler
    }

    #[test]
    fn voice_v2_strips_common_wake_transcription_variants() {
        assert_eq!(
            strip_voice_v2_wake_prefix("Lichlaus, can you hear me?"),
            "can you hear me?"
        );
        assert_eq!(
            strip_voice_v2_wake_prefix("Lich Nos can you hear me"),
            "can you hear me"
        );
        assert_eq!(
            strip_voice_v2_wake_prefix("Lychnos. What time is it?"),
            "What time is it?"
        );
    }

    #[test]
    fn silence_alone_does_not_trigger_initiative() {
        let runtime = runtime();
        let mut scheduler = InitiativeScheduler::new();
        scheduler.started_at =
            Instant::now() - INITIATIVE_IDLE_BEFORE_CHECK - Duration::from_secs(1);

        assert!(scheduler.context_if_due(&runtime, true).is_none());
    }

    #[test]
    fn ambient_observation_can_trigger_without_session_context() {
        let runtime = runtime();
        let mut scheduler = InitiativeScheduler::new();
        scheduler.started_at =
            Instant::now() - INITIATIVE_IDLE_BEFORE_CHECK - Duration::from_secs(1);
        scheduler.note_observation(
            InitiativeTrigger::DiagnosticChange,
            InitiativeObservation {
                source: "omarchy.systemd.user".into(),
                kind: "system.user_services.failed".into(),
                summary: "example.service failed.".into(),
            },
        );

        let context = scheduler
            .context_if_due(&runtime, false)
            .expect("ambient observation should be enough context");
        assert_eq!(context.trigger, InitiativeTrigger::DiagnosticChange);
        assert_eq!(context.observations.len(), 1);
        assert!(context.observations[0].summary.contains("example.service"));
    }

    #[test]
    fn initiative_requires_real_session_context_and_meaningful_trigger() {
        let runtime = runtime();
        let scheduler = idle_scheduler_with_trigger(InitiativeTrigger::ConversationFollowUp);

        assert!(scheduler.context_if_due(&runtime, false).is_none());
        let context = scheduler
            .context_if_due(&runtime, true)
            .expect("meaningful triggered context should be eligible");
        assert_eq!(context.trigger, InitiativeTrigger::ConversationFollowUp);
    }

    #[test]
    fn initiative_scheduler_hard_suppresses_game_mode_and_disabled() {
        let mut runtime = runtime();
        let scheduler = idle_scheduler_with_trigger(InitiativeTrigger::ContextChange);

        runtime.enter_game_mode();
        assert!(scheduler.context_if_due(&runtime, true).is_none());

        runtime.disable();
        assert!(scheduler.context_if_due(&runtime, true).is_none());
    }

    #[test]
    fn unchanged_context_is_considered_only_once() {
        let runtime = runtime();
        let mut scheduler = idle_scheduler_with_trigger(InitiativeTrigger::ConversationFollowUp);

        assert!(scheduler.context_if_due(&runtime, true).is_some());
        scheduler.mark_checked();
        assert!(scheduler.context_if_due(&runtime, true).is_none());

        scheduler.note_context_change(InitiativeTrigger::MemoryCue);
        let context = scheduler
            .context_if_due(&runtime, true)
            .expect("new meaningful context should reopen consideration");
        assert_eq!(context.trigger, InitiativeTrigger::MemoryCue);
    }

    #[test]
    fn recent_user_activity_delays_a_meaningful_trigger() {
        let runtime = runtime();
        let mut scheduler = idle_scheduler_with_trigger(InitiativeTrigger::ConversationFollowUp);
        scheduler.note_user_activity();

        assert!(scheduler.context_if_due(&runtime, true).is_none());
    }
}
