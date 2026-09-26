//! Omarchy live-perception adapter for Lychnos Voice V2.
//!
//! This process owns no AI/persona or execution authority. It converts a local
//! microphone stream into wake/utterance events and obeys session-lifecycle
//! controls from the Lychnos runtime.

use std::{
    collections::VecDeque,
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lychnos_core::perception::{
    LiveSessionController, LiveSessionEvent, LiveSessionState, PerceptionControl, PerceptionEvent,
    PerceptionUtteranceSource,
};
use sherpa_onnx::{
    KeywordSpotter, KeywordSpotterConfig, SpeakerEmbeddingExtractor,
    SpeakerEmbeddingExtractorConfig,
};
use webrtc_vad::{SampleRate, Vad, VadMode};

const SAMPLE_RATE: i32 = 16_000;
const KWS_CHUNK_MS: usize = 100;
const KWS_CHUNK_SAMPLES: usize = SAMPLE_RATE as usize * KWS_CHUNK_MS / 1000;
const KWS_CHUNK_BYTES: usize = KWS_CHUNK_SAMPLES * 2;
const VAD_FRAME_MS: u64 = 20;
const VAD_FRAME_SAMPLES: usize = SAMPLE_RATE as usize * VAD_FRAME_MS as usize / 1000;
const PRE_ROLL_MS: usize = 1_200;
const PRE_ROLL_SAMPLES: usize = SAMPLE_RATE as usize * PRE_ROLL_MS / 1000;
const END_SILENCE_MS: u64 = 850;
const MIN_SPEECH_MS: u64 = 240;
const MAX_UTTERANCE_MS: u64 = 30_000;
const FOLLOW_UP_INPUT_COOLDOWN_MS: u64 = 350;
const FOLLOW_UP_ONSET_SPEECH_MS: u64 = 160;
const SPEAKER_VERIFY_MIN_SPEECH_MS: u64 = 500;
const DEFAULT_SPEAKER_THRESHOLD: f32 = 0.35;

fn main() -> Result<(), String> {
    let model_root = model_root();
    let device = env::var("LYCHNOS_AUDIO_SOURCE")
        .unwrap_or_else(|_| "alsa_input.pci-0000_c5_00.6.analog-stereo".to_string());
    let follow_up_seconds = env::var("LYCHNOS_FOLLOW_UP_SECONDS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(10.0)
        .max(0.0);

    let kws = build_keyword_spotter(&model_root)?;
    let stream = kws.create_stream();
    let speaker_extractor = build_speaker_extractor(&model_root)?;
    let speaker_threshold = env::var("LYCHNOS_FOLLOW_UP_SPEAKER_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(DEFAULT_SPEAKER_THRESHOLD)
        .clamp(0.0, 1.0);
    let mut vad = Vad::new_with_rate(SampleRate::Rate16kHz);
    vad.set_mode(VadMode::Aggressive);
    let control_rx = start_control_reader();

    let mut child = Command::new("parec")
        .args([
            "--record",
            "--device",
            &device,
            "--rate",
            "16000",
            "--format",
            "s16le",
            "--channels",
            "1",
            "--raw",
            "--latency-msec",
            "100",
            "--process-time-msec",
            "100",
            "--client-name",
            "Lychnos Voice V2",
            "--stream-name",
            "Lychnos perception stream",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to start microphone stream: {error}"))?;

    let mut microphone = child
        .stdout
        .take()
        .ok_or_else(|| "microphone stream has no stdout".to_string())?;

    emit(&PerceptionEvent::Ready {
        wake_phrase: "Lychnos".into(),
        device: device.clone(),
    });
    emit_state(LiveSessionState::WakeArmed);

    let mut session = LiveSessionController::new();
    let mut ring = VecDeque::<i16>::with_capacity(PRE_ROLL_SAMPLES);
    let mut capture = Vec::<i16>::new();
    let mut capture_source = PerceptionUtteranceSource::Wake;
    let mut voice_seen = false;
    let mut speech_ms = 0_u64;
    let mut trailing_silence_ms = 0_u64;
    let mut follow_up_deadline: Option<Instant> = None;
    let mut follow_up_ready_at: Option<Instant> = None;
    let mut follow_up_onset_speech_ms = 0_u64;
    let mut follow_up_speaker_verified = false;
    let mut session_speaker_embedding: Option<Vec<f32>> = None;
    let mut buffer = [0_u8; KWS_CHUNK_BYTES];

    loop {
        if process_controls(
            &control_rx,
            &mut session,
            &kws,
            &stream,
            follow_up_seconds,
            &mut follow_up_deadline,
            &mut follow_up_ready_at,
            &mut ring,
            &mut capture,
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Voice V2 runtime control channel closed; perception exiting".into());
        }

        if matches!(
            session.state(),
            LiveSessionState::WakeArmed | LiveSessionState::Disabled
        ) {
            session_speaker_embedding = None;
            follow_up_speaker_verified = false;
            follow_up_onset_speech_ms = 0;
        }

        if session.state() == LiveSessionState::FollowUp
            && follow_up_deadline.is_some_and(|deadline| Instant::now() >= deadline)
        {
            session
                .apply(LiveSessionEvent::FollowUpExpired)
                .map_err(|error| format!("invalid follow-up expiry transition: {error:?}"))?;
            follow_up_deadline = None;
            follow_up_ready_at = None;
            follow_up_onset_speech_ms = 0;
            follow_up_speaker_verified = false;
            session_speaker_embedding = None;
            ring.clear();
            kws.reset(&stream);
            emit(&PerceptionEvent::FollowUpExpired);
            emit_state(session.state());
        }

        microphone
            .read_exact(&mut buffer)
            .map_err(|error| format!("microphone stream ended: {error}"))?;

        let samples = decode_s16le(&buffer);

        if matches!(
            session.state(),
            LiveSessionState::WakeArmed | LiveSessionState::FollowUp
        ) {
            for sample in &samples {
                ring.push_back(*sample);
            }
            while ring.len() > PRE_ROLL_SAMPLES {
                ring.pop_front();
            }
        }

        match session.state() {
            LiveSessionState::WakeArmed => {
                let normalized: Vec<f32> = samples
                    .iter()
                    .map(|sample| f32::from(*sample) / 32768.0)
                    .collect();
                stream.accept_waveform(SAMPLE_RATE, &normalized);

                while kws.is_ready(&stream) {
                    kws.decode(&stream);
                    let Some(result) = kws.get_result(&stream) else {
                        continue;
                    };
                    if result.keyword.is_empty() {
                        continue;
                    }

                    let keyword = result.keyword.clone();
                    kws.reset(&stream);
                    session
                        .apply(LiveSessionEvent::WakeDetected)
                        .map_err(|error| format!("invalid wake transition: {error:?}"))?;

                    capture.clear();
                    capture.extend(ring.iter().copied());
                    capture_source = PerceptionUtteranceSource::Wake;
                    // The keyword itself is known speech. Seeding this lets
                    // "Lychnos" alone endpoint cleanly after trailing silence,
                    // while immediate speech continues the same utterance.
                    voice_seen = true;
                    speech_ms = MIN_SPEECH_MS;
                    trailing_silence_ms = 0;
                    ring.clear();

                    emit(&PerceptionEvent::WakeDetected { keyword });
                    emit_state(session.state());
                    break;
                }
            }
            LiveSessionState::FollowUp => {
                if follow_up_ready_at.is_some_and(|ready| Instant::now() < ready) {
                    continue;
                }

                let (frames, _) = samples.as_chunks::<VAD_FRAME_SAMPLES>();
                for frame in frames {
                    if vad.is_voice_segment(frame).unwrap_or(false) {
                        follow_up_onset_speech_ms =
                            follow_up_onset_speech_ms.saturating_add(VAD_FRAME_MS);
                    } else {
                        follow_up_onset_speech_ms = 0;
                    }
                }

                if follow_up_onset_speech_ms >= FOLLOW_UP_ONSET_SPEECH_MS {
                    session
                        .apply(LiveSessionEvent::FollowUpSpeechDetected)
                        .map_err(|error| {
                            format!("invalid follow-up speech transition: {error:?}")
                        })?;
                    capture.clear();
                    capture.extend(ring.iter().copied());
                    capture_source = PerceptionUtteranceSource::FollowUp;
                    voice_seen = true;
                    speech_ms = follow_up_onset_speech_ms;
                    trailing_silence_ms = 0;
                    follow_up_onset_speech_ms = 0;
                    follow_up_speaker_verified = session_speaker_embedding.is_none();
                    follow_up_ready_at = None;
                    ring.clear();
                    emit_state(session.state());
                }
            }
            LiveSessionState::Listening => {
                capture.extend_from_slice(&samples);
                let (frames, _) = samples.as_chunks::<VAD_FRAME_SAMPLES>();
                for frame in frames {
                    let speech = vad.is_voice_segment(frame).unwrap_or(false);
                    if speech {
                        voice_seen = true;
                        speech_ms = speech_ms.saturating_add(VAD_FRAME_MS);
                        trailing_silence_ms = 0;
                    } else if voice_seen {
                        trailing_silence_ms = trailing_silence_ms.saturating_add(VAD_FRAME_MS);
                    }
                }

                let duration_ms = samples_to_ms(capture.len());

                if capture_source == PerceptionUtteranceSource::FollowUp
                    && !follow_up_speaker_verified
                    && speech_ms >= SPEAKER_VERIFY_MIN_SPEECH_MS
                    && let Some(reference) = session_speaker_embedding.as_deref()
                    && let Some(query) = compute_speaker_embedding(&speaker_extractor, &capture)
                {
                    let similarity = cosine_similarity(reference, &query);
                    if similarity >= speaker_threshold {
                        follow_up_speaker_verified = true;
                        follow_up_deadline = None;
                        eprintln!(
                            "Voice V2 follow-up speaker accepted · similarity={similarity:.3}"
                        );
                    } else {
                        reject_follow_up_speaker(
                            &mut session,
                            similarity,
                            &mut capture,
                            &mut voice_seen,
                            &mut speech_ms,
                            &mut trailing_silence_ms,
                            &mut follow_up_ready_at,
                            &mut follow_up_speaker_verified,
                            &mut ring,
                        )?;
                        continue;
                    }
                }

                let end_of_turn = voice_seen
                    && speech_ms >= MIN_SPEECH_MS
                    && trailing_silence_ms >= END_SILENCE_MS;
                let safety_limit = duration_ms >= MAX_UTTERANCE_MS;

                if end_of_turn || safety_limit {
                    if capture_source == PerceptionUtteranceSource::FollowUp
                        && !follow_up_speaker_verified
                        && let Some(reference) = session_speaker_embedding.as_deref()
                    {
                        let similarity = compute_speaker_embedding(&speaker_extractor, &capture)
                            .map(|query| cosine_similarity(reference, &query));
                        match similarity {
                            Some(score) if score >= speaker_threshold => {
                                eprintln!(
                                    "Voice V2 short follow-up speaker accepted · similarity={score:.3}"
                                );
                            }
                            Some(score) => {
                                reject_follow_up_speaker(
                                    &mut session,
                                    score,
                                    &mut capture,
                                    &mut voice_seen,
                                    &mut speech_ms,
                                    &mut trailing_silence_ms,
                                    &mut follow_up_ready_at,
                                    &mut follow_up_speaker_verified,
                                    &mut ring,
                                )?;
                                continue;
                            }
                            None => {
                                eprintln!(
                                    "Voice V2 follow-up too short for speaker verification; ignoring"
                                );
                                reject_follow_up_speaker(
                                    &mut session,
                                    0.0,
                                    &mut capture,
                                    &mut voice_seen,
                                    &mut speech_ms,
                                    &mut trailing_silence_ms,
                                    &mut follow_up_ready_at,
                                    &mut follow_up_speaker_verified,
                                    &mut ring,
                                )?;
                                continue;
                            }
                        }
                    }

                    if capture_source == PerceptionUtteranceSource::Wake {
                        session_speaker_embedding =
                            compute_speaker_embedding(&speaker_extractor, &capture);
                        if session_speaker_embedding.is_some() {
                            eprintln!(
                                "Voice V2 session speaker fingerprint ready · ephemeral=true"
                            );
                        } else {
                            eprintln!(
                                "Voice V2 session speaker fingerprint unavailable; follow-up verification degraded"
                            );
                        }
                    }

                    session
                        .apply(LiveSessionEvent::UtteranceFinalized)
                        .map_err(|error| format!("invalid utterance transition: {error:?}"))?;
                    let path = utterance_path();
                    write_wav(&path, &capture)?;
                    emit(&PerceptionEvent::UtteranceFinalized {
                        path: path.display().to_string(),
                        duration_ms,
                        source: capture_source,
                    });
                    emit_state(session.state());

                    capture.clear();
                    voice_seen = false;
                    speech_ms = 0;
                    trailing_silence_ms = 0;
                    follow_up_speaker_verified = false;
                }
            }
            LiveSessionState::Thinking
            | LiveSessionState::Speaking
            | LiveSessionState::Disabled => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_controls(
    receiver: &Receiver<PerceptionControl>,
    session: &mut LiveSessionController,
    kws: &KeywordSpotter,
    stream: &sherpa_onnx::OnlineStream,
    follow_up_seconds: f64,
    follow_up_deadline: &mut Option<Instant>,
    follow_up_ready_at: &mut Option<Instant>,
    ring: &mut VecDeque<i16>,
    capture: &mut Vec<i16>,
) -> bool {
    loop {
        let control = match receiver.try_recv() {
            Ok(control) => control,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => return true,
        };

        let result = match control {
            PerceptionControl::Enable => session.apply(LiveSessionEvent::Enable),
            PerceptionControl::Disable => session.apply(LiveSessionEvent::Disable),
            PerceptionControl::ResponseStarted => session.apply(LiveSessionEvent::ResponseStarted),
            PerceptionControl::ResponseFinished { follow_up } => {
                session.apply(LiveSessionEvent::ResponseFinished { follow_up })
            }
        };

        match result {
            Ok(state) => {
                ring.clear();
                capture.clear();
                kws.reset(stream);

                if state == LiveSessionState::FollowUp && follow_up_seconds > 0.0 {
                    let now = Instant::now();
                    *follow_up_deadline = Some(now + Duration::from_secs_f64(follow_up_seconds));
                    *follow_up_ready_at =
                        Some(now + Duration::from_millis(FOLLOW_UP_INPUT_COOLDOWN_MS));
                } else {
                    *follow_up_deadline = None;
                    *follow_up_ready_at = None;
                }

                emit_state(state);
            }
            Err(error) => {
                eprintln!(
                    "ignoring invalid perception control {control:?} in {:?}: {error:?}",
                    session.state()
                );
            }
        }
    }
}

fn start_control_reader() -> Receiver<PerceptionControl> {
    let (sender, receiver) = mpsc::channel();

    thread::Builder::new()
        .name("lychnos-perception-control".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            for line in BufReader::new(stdin.lock()).lines() {
                let Ok(line) = line else {
                    break;
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                match serde_json::from_str::<PerceptionControl>(trimmed) {
                    Ok(control) => {
                        if sender.send(control).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("invalid perception control {trimmed:?}: {error}");
                    }
                }
            }
        })
        .expect("perception control reader thread should start");

    receiver
}

fn build_speaker_extractor(model_root: &Path) -> Result<SpeakerEmbeddingExtractor, String> {
    let speaker_model = env::var_os("LYCHNOS_SPEAKER_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            model_root
                .parent()
                .unwrap_or(model_root)
                .join("3dspeaker_speech_campplus_sv_zh-cn_16k-common.onnx")
        });
    let config = SpeakerEmbeddingExtractorConfig {
        model: Some(speaker_model.display().to_string()),
        num_threads: 1,
        debug: false,
        provider: Some("cpu".into()),
    };

    SpeakerEmbeddingExtractor::create(&config)
        .ok_or_else(|| "failed to create Voice V2 speaker embedding extractor".to_string())
}

fn compute_speaker_embedding(
    extractor: &SpeakerEmbeddingExtractor,
    samples: &[i16],
) -> Option<Vec<f32>> {
    if samples.is_empty() {
        return None;
    }

    let normalized: Vec<f32> = samples
        .iter()
        .map(|sample| f32::from(*sample) / 32768.0)
        .collect();
    let stream = extractor.create_stream()?;
    stream.accept_waveform(SAMPLE_RATE, &normalized);
    stream.input_finished();

    extractor
        .is_ready(&stream)
        .then(|| extractor.compute(&stream))
        .flatten()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }

    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();

    dot / (left_norm * right_norm).max(f32::EPSILON)
}

#[allow(clippy::too_many_arguments)]
fn reject_follow_up_speaker(
    session: &mut LiveSessionController,
    similarity: f32,
    capture: &mut Vec<i16>,
    voice_seen: &mut bool,
    speech_ms: &mut u64,
    trailing_silence_ms: &mut u64,
    follow_up_ready_at: &mut Option<Instant>,
    follow_up_speaker_verified: &mut bool,
    ring: &mut VecDeque<i16>,
) -> Result<(), String> {
    session
        .apply(LiveSessionEvent::FollowUpRejected)
        .map_err(|error| format!("invalid rejected-speaker transition: {error:?}"))?;

    let score_milli = (similarity.clamp(0.0, 1.0) * 1000.0).round() as u16;
    emit(&PerceptionEvent::FollowUpSpeakerRejected { score_milli });
    emit_state(session.state());
    eprintln!("Voice V2 ignored non-session speaker · similarity={similarity:.3}");

    capture.clear();
    *voice_seen = false;
    *speech_ms = 0;
    *trailing_silence_ms = 0;
    *follow_up_speaker_verified = false;
    *follow_up_ready_at = Some(Instant::now() + Duration::from_millis(FOLLOW_UP_INPUT_COOLDOWN_MS));
    ring.clear();
    Ok(())
}

fn build_keyword_spotter(model_root: &Path) -> Result<KeywordSpotter, String> {
    let mut config = KeywordSpotterConfig::default();
    config.model_config.transducer.encoder = Some(
        model_root
            .join("encoder-epoch-13-avg-2-chunk-8-left-64.int8.onnx")
            .display()
            .to_string(),
    );
    config.model_config.transducer.decoder = Some(
        model_root
            .join("decoder-epoch-13-avg-2-chunk-8-left-64.onnx")
            .display()
            .to_string(),
    );
    config.model_config.transducer.joiner = Some(
        model_root
            .join("joiner-epoch-13-avg-2-chunk-8-left-64.int8.onnx")
            .display()
            .to_string(),
    );
    config.model_config.tokens = Some(model_root.join("tokens.txt").display().to_string());
    config.model_config.provider = Some("cpu".to_string());
    config.model_config.num_threads = 1;
    config.model_config.debug = false;
    config.max_active_paths = 4;
    config.num_trailing_blanks = 1;
    config.keywords_score = 3.0;
    config.keywords_threshold = 0.10;
    config.keywords_file = Some(
        model_root
            .join("lychnos-keywords.txt")
            .display()
            .to_string(),
    );

    KeywordSpotter::create(&config).ok_or_else(|| {
        format!(
            "failed to create keyword spotter from {}",
            model_root.display()
        )
    })
}

fn model_root() -> PathBuf {
    if let Some(path) = env::var_os("LYCHNOS_KWS_MODEL_DIR") {
        return PathBuf::from(path);
    }
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(
        " .local/share/lychnos/models/voice-v2/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20"
            .trim(),
    )
}

fn decode_s16le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect()
}

fn samples_to_ms(samples: usize) -> u64 {
    ((samples as u128 * 1_000) / SAMPLE_RATE as u128) as u64
}

fn utterance_path() -> PathBuf {
    let root = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    root.join("lychnos/voice-v2").join(format!(
        "utterance-{}-{}.wav",
        std::process::id(),
        unique_nanos()
    ))
}

fn write_wav(path: &Path, samples: &[i16]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

    let data_bytes = u32::try_from(samples.len().saturating_mul(2))
        .map_err(|_| "voice-v2 WAV is too large".to_string())?;
    let riff_bytes = 36_u32
        .checked_add(data_bytes)
        .ok_or_else(|| "voice-v2 WAV size overflow".to_string())?;
    let mut file =
        fs::File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;

    file.write_all(b"RIFF")
        .and_then(|_| file.write_all(&riff_bytes.to_le_bytes()))
        .and_then(|_| file.write_all(b"WAVEfmt "))
        .and_then(|_| file.write_all(&16_u32.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&(SAMPLE_RATE as u32).to_le_bytes()))
        .and_then(|_| file.write_all(&((SAMPLE_RATE as u32) * 2).to_le_bytes()))
        .and_then(|_| file.write_all(&2_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&16_u16.to_le_bytes()))
        .and_then(|_| file.write_all(b"data"))
        .and_then(|_| file.write_all(&data_bytes.to_le_bytes()))
        .map_err(|error| format!("write {} header: {error}", path.display()))?;

    for sample in samples {
        file.write_all(&sample.to_le_bytes())
            .map_err(|error| format!("write {} samples: {error}", path.display()))?;
    }
    file.flush()
        .map_err(|error| format!("flush {}: {error}", path.display()))
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn emit(event: &PerceptionEvent) {
    match serde_json::to_string(event) {
        Ok(json) => println!("{json}"),
        Err(error) => eprintln!("voice-v2 event serialization failed: {error}"),
    }
}

fn emit_state(state: LiveSessionState) {
    emit(&PerceptionEvent::State { state });
}
