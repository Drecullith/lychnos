use std::{
    collections::VecDeque,
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lychnos_core::{
    interaction::InteractionSource,
    speech::SpeechToTextProvider,
    voice::{AudioInputDevice, VoiceCaptureId},
};

use crate::stt::WhisperCppStt;

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_MS: usize = 100;
const SAMPLES_PER_CHUNK: usize = SAMPLE_RATE as usize * CHUNK_MS / 1000;
const BYTES_PER_CHUNK: usize = SAMPLES_PER_CHUNK * 2;
const PRE_ROLL_CHUNKS: usize = 6;
const WAKE_END_SILENCE_CHUNKS: usize = 5;
const MAX_SEGMENT_CHUNKS: usize = 300;
const MIN_SPEECH_CHUNKS: usize = 2;
const CALIBRATION_CHUNKS: usize = 12;
const DEFAULT_FOLLOW_UP_WINDOW: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeWordEvent {
    Activated {
        capture_id: VoiceCaptureId,
        transcript: String,
    },
    Command {
        capture_id: VoiceCaptureId,
        source: InteractionSource,
        transcript: String,
        command: String,
    },
    SessionExpired,
    Error(String),
}

pub struct WakeWordWorker {
    receiver: Receiver<WakeWordEvent>,
    suspended: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    follow_up_deadline: Arc<Mutex<Option<Instant>>>,
}

impl WakeWordWorker {
    pub fn start(stt: WhisperCppStt, device: AudioInputDevice) -> Result<Self, String> {
        ensure_parec_available()?;

        let (sender, receiver) = mpsc::channel();
        let suspended = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let follow_up_deadline = Arc::new(Mutex::new(None));

        let worker_suspended = Arc::clone(&suspended);
        let worker_stop = Arc::clone(&stop);
        let worker_deadline = Arc::clone(&follow_up_deadline);

        thread::Builder::new()
            .name("lychnos-wake-word".into())
            .spawn(move || {
                if let Err(error) = run_wake_worker(
                    stt,
                    device,
                    worker_suspended,
                    worker_stop,
                    worker_deadline,
                    &sender,
                ) {
                    let _ = sender.send(WakeWordEvent::Error(error));
                }
            })
            .map_err(|error| format!("failed to start wake-word worker: {error}"))?;

        Ok(Self {
            receiver,
            suspended,
            stop,
            follow_up_deadline,
        })
    }

    pub fn try_recv(&self) -> Option<WakeWordEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn set_suspended(&self, suspended: bool) {
        self.suspended.store(suspended, Ordering::Release);
    }

    pub fn open_follow_up_window(&self) {
        if let Ok(mut deadline) = self.follow_up_deadline.lock() {
            *deadline = Some(Instant::now() + DEFAULT_FOLLOW_UP_WINDOW);
        }
    }

    pub fn clear_follow_up_window(&self) {
        if let Ok(mut deadline) = self.follow_up_deadline.lock() {
            *deadline = None;
        }
    }
}

impl Drop for WakeWordWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn run_wake_worker(
    stt: WhisperCppStt,
    device: AudioInputDevice,
    suspended: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    follow_up_deadline: Arc<Mutex<Option<Instant>>>,
    sender: &Sender<WakeWordEvent>,
) -> Result<(), String> {
    let mut child = Command::new("parec")
        .args([
            "--record",
            "--device",
            device.id.as_str(),
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
            "Lychnos Wake Word",
            "--stream-name",
            "Local wake-word listener",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to start local wake-word microphone stream: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "wake-word microphone stream has no stdout".to_string())?;
    let mut gate = SpeechGate::new();
    let mut bytes = [0_u8; BYTES_PER_CHUNK];
    let mut session_was_active = false;

    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }

        stdout
            .read_exact(&mut bytes)
            .map_err(|error| format!("wake-word microphone stream ended: {error}"))?;

        if suspended.load(Ordering::Acquire) {
            gate.reset();
            continue;
        }

        let session_active = follow_up_is_active(&follow_up_deadline);
        if session_was_active && !session_active {
            let _ = sender.send(WakeWordEvent::SessionExpired);
        }
        session_was_active = session_active;

        let samples = decode_s16le(&bytes);
        let Some(segment) = gate.push(&samples, session_active) else {
            continue;
        };

        println!(
            "Wake audio segment · mode={} · {:.1}s",
            if session_active { "follow-up" } else { "wake" },
            segment.len() as f64 / SAMPLE_RATE as f64
        );

        let path = wake_audio_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create wake audio directory: {error}"))?;
        }

        write_wav(&path, &segment)?;

        let transcript_result = stt.transcribe(&path);
        if let Err(error) = fs::remove_file(&path) {
            eprintln!(
                "Failed to delete ephemeral wake audio {}: {error}",
                path.display()
            );
        }

        let transcript = match transcript_result {
            Ok(transcript) => transcript.text.trim().to_string(),
            Err(error) => {
                eprintln!("Wake-word Whisper pass failed: {error}");
                continue;
            }
        };

        if transcript.is_empty() {
            continue;
        }

        if follow_up_is_active(&follow_up_deadline) {
            clear_deadline(&follow_up_deadline);
            session_was_active = false;
            let _ = sender.send(WakeWordEvent::Command {
                capture_id: new_wake_capture_id(),
                source: InteractionSource::VoiceSession,
                transcript: transcript.clone(),
                command: transcript,
            });
            continue;
        }

        let Some(command) = extract_wake_command(&transcript) else {
            continue;
        };

        let capture_id = new_wake_capture_id();
        let _ = sender.send(WakeWordEvent::Activated {
            capture_id: capture_id.clone(),
            transcript: transcript.clone(),
        });

        if command.is_empty() {
            set_deadline(&follow_up_deadline, DEFAULT_FOLLOW_UP_WINDOW);
            session_was_active = true;
        } else {
            clear_deadline(&follow_up_deadline);
            session_was_active = false;
            let _ = sender.send(WakeWordEvent::Command {
                capture_id,
                source: InteractionSource::WakeWord,
                transcript,
                command,
            });
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn ensure_parec_available() -> Result<(), String> {
    Command::new("parec")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("parec is unavailable for wake-word capture: {error}"))?;
    Ok(())
}

fn follow_up_is_active(deadline: &Arc<Mutex<Option<Instant>>>) -> bool {
    let Ok(mut deadline) = deadline.lock() else {
        return false;
    };

    match *deadline {
        Some(until) if Instant::now() < until => true,
        Some(_) => {
            *deadline = None;
            false
        }
        None => false,
    }
}

fn set_deadline(deadline: &Arc<Mutex<Option<Instant>>>, duration: Duration) {
    if let Ok(mut deadline) = deadline.lock() {
        *deadline = Some(Instant::now() + duration);
    }
}

fn clear_deadline(deadline: &Arc<Mutex<Option<Instant>>>) {
    if let Ok(mut deadline) = deadline.lock() {
        *deadline = None;
    }
}

fn new_wake_capture_id() -> VoiceCaptureId {
    VoiceCaptureId::new(format!("wake-{}-{}", std::process::id(), unique_nanos()))
}

fn wake_audio_path() -> PathBuf {
    let root = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    root.join("lychnos/audio").join(format!(
        "wake-{}-{}.wav",
        std::process::id(),
        unique_nanos()
    ))
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn decode_s16le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect()
}

fn write_wav(path: &Path, samples: &[i16]) -> Result<(), String> {
    let data_bytes = samples
        .len()
        .checked_mul(2)
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or_else(|| "wake-word audio segment is too large".to_string())?;
    let riff_bytes = 36_u32
        .checked_add(data_bytes)
        .ok_or_else(|| "wake-word WAV size overflow".to_string())?;

    let mut file = fs::File::create(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;

    file.write_all(b"RIFF")
        .and_then(|_| file.write_all(&riff_bytes.to_le_bytes()))
        .and_then(|_| file.write_all(b"WAVE"))
        .and_then(|_| file.write_all(b"fmt "))
        .and_then(|_| file.write_all(&16_u32.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&1_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&SAMPLE_RATE.to_le_bytes()))
        .and_then(|_| file.write_all(&(SAMPLE_RATE * 2).to_le_bytes()))
        .and_then(|_| file.write_all(&2_u16.to_le_bytes()))
        .and_then(|_| file.write_all(&16_u16.to_le_bytes()))
        .and_then(|_| file.write_all(b"data"))
        .and_then(|_| file.write_all(&data_bytes.to_le_bytes()))
        .map_err(|error| format!("failed to write WAV header: {error}"))?;

    for sample in samples {
        file.write_all(&sample.to_le_bytes())
            .map_err(|error| format!("failed to write WAV audio: {error}"))?;
    }

    file.flush()
        .map_err(|error| format!("failed to flush wake WAV: {error}"))
}

pub fn should_close_session(text: &str) -> bool {
    let normalized = normalized_words(text).join(" ");

    let closing_suffix = [
        "goodbye",
        "bye",
        "talk later",
        "speak later",
        "see you later",
        "catch you later",
    ];
    if closing_suffix
        .iter()
        .any(|phrase| normalized == *phrase || normalized.ends_with(&format!(" {phrase}")))
    {
        return true;
    }

    [
        "that s all",
        "that is all",
        "stand down",
        "stop listening",
        "go idle",
        "that s enough for now",
        "that is enough for now",
        "we can stop for now",
        "i m going back to",
        "i am going back to",
        "going back to work",
        "going back to fixing",
        "back to work for me",
        "back to fixing for me",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
}

pub fn play_acknowledgement() -> Result<(), String> {
    let path = wake_audio_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create wake audio directory: {error}"))?;
    }

    let samples = acknowledgement_samples();
    write_wav(&path, &samples)?;

    let status = Command::new("pw-play")
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to play wake acknowledgement: {error}"));

    if let Err(error) = fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "Failed to delete wake acknowledgement {}: {error}",
            path.display()
        );
    }

    let status = status?;
    if !status.success() {
        return Err(format!(
            "wake acknowledgement playback exited with {status}"
        ));
    }

    Ok(())
}

fn acknowledgement_samples() -> Vec<i16> {
    const TOTAL_MS: usize = 120;
    const FIRST_TONE_MS: usize = 55;
    const AMPLITUDE: f32 = 2_200.0;

    let total_samples = SAMPLE_RATE as usize * TOTAL_MS / 1_000;
    let first_tone_samples = SAMPLE_RATE as usize * FIRST_TONE_MS / 1_000;

    (0..total_samples)
        .map(|index| {
            let frequency = if index < first_tone_samples {
                660.0
            } else {
                880.0
            };
            let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / SAMPLE_RATE as f32;
            let edge = 80_usize.min(total_samples / 4).max(1);
            let fade_in = (index as f32 / edge as f32).min(1.0);
            let fade_out = ((total_samples - 1 - index) as f32 / edge as f32).min(1.0);
            let envelope = fade_in.min(fade_out);
            (phase.sin() * AMPLITUDE * envelope) as i16
        })
        .collect()
}

fn extract_wake_command(transcript: &str) -> Option<String> {
    let words = normalized_words(transcript);

    for index in 0..words.len() {
        let Some(length) = wake_alias_length(&words, index) else {
            continue;
        };

        let mut command_start = index + length;

        // If the user repeats the wake name for emphasis, strip consecutive
        // repeated wake aliases instead of forwarding them to the brain.
        while let Some(repeated_length) = wake_alias_length(&words, command_start) {
            command_start += repeated_length;
        }

        return Some(words[command_start..].join(" "));
    }

    None
}

fn wake_alias_length(words: &[String], index: usize) -> Option<usize> {
    const SINGLE_ALIASES: [&str; 11] = [
        "lychnos",
        "lichnos",
        "liknos",
        "lyknos",
        "lechnos",
        "leeknos",
        "leechnos",
        "leehnos",
        "lihnos",
        "lixnos",
        "λύχνος",
    ];
    const DOUBLE_ALIASES: [[&str; 2]; 5] = [
        ["leek", "nos"],
        ["lick", "nos"],
        ["lich", "nos"],
        ["lee", "nos"],
        ["li", "nos"],
    ];
    const TRIPLE_ALIASES: [[&str; 3]; 4] = [
        ["lee", "h", "nos"],
        ["lee", "kh", "nos"],
        ["li", "h", "nos"],
        ["li", "kh", "nos"],
    ];

    let word = words.get(index)?;
    if SINGLE_ALIASES.contains(&word.as_str()) {
        return Some(1);
    }

    if index + 1 < words.len()
        && DOUBLE_ALIASES
            .iter()
            .any(|alias| words[index] == alias[0] && words[index + 1] == alias[1])
    {
        return Some(2);
    }

    if index + 2 < words.len()
        && TRIPLE_ALIASES.iter().any(|alias| {
            words[index] == alias[0] && words[index + 1] == alias[1] && words[index + 2] == alias[2]
        })
    {
        return Some(3);
    }

    None
}

fn normalized_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

struct SpeechGate {
    noise_floor: f32,
    calibration_chunks: usize,
    pre_roll: VecDeque<Vec<i16>>,
    active: Option<Vec<i16>>,
    speech_chunks: usize,
    trailing_silence_chunks: usize,
}

impl SpeechGate {
    fn new() -> Self {
        Self {
            noise_floor: 120.0,
            calibration_chunks: 0,
            pre_roll: VecDeque::new(),
            active: None,
            speech_chunks: 0,
            trailing_silence_chunks: 0,
        }
    }

    fn reset(&mut self) {
        self.pre_roll.clear();
        self.active = None;
        self.speech_chunks = 0;
        self.trailing_silence_chunks = 0;
    }

    #[cfg(test)]
    fn calibrated_for_test() -> Self {
        let mut gate = Self::new();
        gate.calibration_chunks = CALIBRATION_CHUNKS;
        gate
    }

    fn push(&mut self, samples: &[i16], relaxed_follow_up: bool) -> Option<Vec<i16>> {
        let rms = rms(samples);

        // Learn the real microphone/room floor before wake detection is armed.
        // Without this, a source whose idle RMS is above the compiled default can
        // immediately enter an endless "speech" segment and never reach Whisper.
        if self.calibration_chunks < CALIBRATION_CHUNKS && self.active.is_none() {
            if self.calibration_chunks == 0 {
                self.noise_floor = rms.max(1.0);
            } else {
                self.noise_floor = self.noise_floor * 0.75 + rms * 0.25;
            }
            self.calibration_chunks += 1;
            self.pre_roll.push_back(samples.to_vec());
            while self.pre_roll.len() > PRE_ROLL_CHUNKS {
                self.pre_roll.pop_front();
            }
            return None;
        }

        // Initial detection is intentionally permissive. A false candidate is
        // harmless: it still has to survive local Whisper transcription and
        // match a Lychnos wake alias before it can wake the companion.
        let start_threshold = if relaxed_follow_up {
            (self.noise_floor * 1.30 + 15.0).max(75.0)
        } else {
            (self.noise_floor * 1.40 + 20.0).max(90.0)
        };
        // Keep enough hysteresis for soft syllables without letting ordinary
        // room noise hold a segment open forever after one loud transient.
        let continue_threshold = if relaxed_follow_up {
            (self.noise_floor * 1.15 + 10.0).max(45.0)
        } else {
            (self.noise_floor * 1.25 + 15.0).max(55.0)
        };

        let speech = if self.active.is_some() {
            rms >= continue_threshold
        } else {
            rms >= start_threshold
        };

        if let Some(active) = self.active.as_mut() {
            active.extend_from_slice(samples);

            if speech {
                self.speech_chunks += 1;
                self.trailing_silence_chunks = 0;
            } else {
                self.trailing_silence_chunks += 1;
            }

            let active_chunks = active.len() / SAMPLES_PER_CHUNK;
            let end_silence_chunks = required_end_silence_chunks(relaxed_follow_up, active_chunks);
            let finished = self.trailing_silence_chunks >= end_silence_chunks
                || active_chunks >= MAX_SEGMENT_CHUNKS;

            if finished {
                let segment = self.active.take().unwrap_or_default();
                let enough_speech = self.speech_chunks >= MIN_SPEECH_CHUNKS;
                self.speech_chunks = 0;
                self.trailing_silence_chunks = 0;
                self.pre_roll.clear();

                if enough_speech {
                    return Some(segment);
                }
            }

            return None;
        }

        if speech {
            let mut active = Vec::with_capacity(SAMPLES_PER_CHUNK * 20);
            for chunk in self.pre_roll.drain(..) {
                active.extend(chunk);
            }
            active.extend_from_slice(samples);
            self.active = Some(active);
            self.speech_chunks = 1;
            self.trailing_silence_chunks = 0;
        } else {
            // Follow a rising room/microphone floor quickly enough that fans,
            // gain changes, or AGC do not become false speech. Loud outliers
            // are deliberately excluded so real speech is not learned away.
            if rms <= self.noise_floor * 1.50 {
                let weight = if rms > self.noise_floor { 0.15 } else { 0.04 };
                self.noise_floor = self.noise_floor * (1.0 - weight) + rms * weight;
            }
            self.pre_roll.push_back(samples.to_vec());
            while self.pre_roll.len() > PRE_ROLL_CHUNKS {
                self.pre_roll.pop_front();
            }
        }

        None
    }
}

fn required_end_silence_chunks(relaxed_follow_up: bool, active_chunks: usize) -> usize {
    if !relaxed_follow_up {
        return WAKE_END_SILENCE_CHUNKS;
    }

    match active_chunks {
        0..=19 => 7,   // <= 2s speech: ~700ms silence
        20..=49 => 10, // <= 5s speech: ~1.0s silence
        _ => 14,       // longer thought: ~1.4s silence
    }
}

fn rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let energy = samples
        .iter()
        .map(|sample| {
            let sample = f64::from(*sample);
            sample * sample
        })
        .sum::<f64>()
        / samples.len() as f64;

    energy.sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_phrase_extracts_inline_command() {
        assert_eq!(
            extract_wake_command("Lychnos, tell me the time."),
            Some("tell me the time".into())
        );
        assert_eq!(
            extract_wake_command("Okay Lichnos what changed?"),
            Some("what changed".into())
        );
    }

    #[test]
    fn greek_style_pronunciation_aliases_are_recognized() {
        assert_eq!(
            extract_wake_command("Lee H Nos, can you hear me?"),
            Some("can you hear me".into())
        );
        assert_eq!(
            extract_wake_command("Lee Kh Nos what changed?"),
            Some("what changed".into())
        );
        assert_eq!(
            extract_wake_command("Leehnos tell me something"),
            Some("tell me something".into())
        );
    }

    #[test]
    fn repeated_wake_name_is_removed_from_command() {
        assert_eq!(
            extract_wake_command("Lich Nos. Lich Nos, how are you?"),
            Some("how are you".into())
        );
        assert_eq!(
            extract_wake_command("Lee H Nos, Lee H Nos, are you there?"),
            Some("are you there".into())
        );
    }

    #[test]
    fn wake_phrase_alone_opens_follow_up() {
        assert_eq!(extract_wake_command("Lychnos"), Some(String::new()));
        assert_eq!(extract_wake_command("Leek Nos!"), Some(String::new()));
    }

    #[test]
    fn unrelated_speech_does_not_wake() {
        assert_eq!(extract_wake_command("this is just background speech"), None);
    }

    #[test]
    fn relaxed_follow_up_threshold_accepts_quieter_speech_than_idle_wake_gate() {
        let quiet_speech = vec![175_i16; SAMPLES_PER_CHUNK];

        let mut strict_gate = SpeechGate::calibrated_for_test();
        assert!(strict_gate.push(&quiet_speech, false).is_none());
        assert!(strict_gate.active.is_none());

        let mut follow_up_gate = SpeechGate::calibrated_for_test();
        assert!(follow_up_gate.push(&quiet_speech, true).is_none());
        assert!(follow_up_gate.active.is_some());
    }

    #[test]
    fn rising_background_noise_is_learned_without_starting_false_speech() {
        let background = vec![600_i16; SAMPLES_PER_CHUNK];
        let mut gate = SpeechGate::calibrated_for_test();
        gate.noise_floor = 500.0;

        for _ in 0..20 {
            assert!(gate.push(&background, false).is_none());
            assert!(gate.active.is_none());
        }

        assert!(gate.noise_floor > 575.0);
    }

    #[test]
    fn continuation_hysteresis_keeps_long_turn_open_through_soft_syllables() {
        let loud_speech = vec![400_i16; SAMPLES_PER_CHUNK];
        let soft_speech = vec![180_i16; SAMPLES_PER_CHUNK];
        let silence = vec![0_i16; SAMPLES_PER_CHUNK];
        let mut gate = SpeechGate::calibrated_for_test();

        assert!(gate.push(&loud_speech, true).is_none());
        assert!(gate.active.is_some());

        for _ in 0..3 {
            assert!(gate.push(&soft_speech, true).is_none());
            assert!(gate.active.is_some());
        }

        for _ in 0..required_end_silence_chunks(true, 4) - 1 {
            assert!(gate.push(&silence, true).is_none());
        }

        assert!(gate.push(&soft_speech, true).is_none());
        assert!(gate.active.is_some());

        let mut emitted = None;
        for _ in 0..required_end_silence_chunks(true, 2) {
            emitted = gate.push(&silence, true).or(emitted);
        }

        assert!(emitted.is_some());
    }

    #[test]
    fn long_turn_is_not_forced_closed_at_the_old_eight_second_limit() {
        let speech = vec![400_i16; SAMPLES_PER_CHUNK];
        let mut gate = SpeechGate::calibrated_for_test();

        for _ in 0..100 {
            assert!(gate.push(&speech, true).is_none());
            assert!(gate.active.is_some());
        }

        assert!(
            gate.active
                .as_ref()
                .is_some_and(|segment| { segment.len() >= SAMPLES_PER_CHUNK * 100 })
        );
    }

    #[test]
    fn endpoint_patience_increases_for_longer_follow_up_turns() {
        assert_eq!(required_end_silence_chunks(true, 10), 7);
        assert_eq!(required_end_silence_chunks(true, 30), 10);
        assert_eq!(required_end_silence_chunks(true, 80), 14);
        assert_eq!(
            required_end_silence_chunks(false, 80),
            WAKE_END_SILENCE_CHUNKS
        );
    }

    #[test]
    fn clear_sign_off_phrases_close_hands_free_session() {
        assert!(should_close_session(
            "Thank you brother, I'm going back to fixing."
        ));
        assert!(should_close_session("That's all, talk later."));
        assert!(should_close_session("Back to work for me."));
        assert!(should_close_session("You can stand down for now, brother."));
        assert!(should_close_session("Stop listening, I'll call you later."));
        assert!(!should_close_session("Thank you brother."));
        assert!(!should_close_session(
            "Can we talk later about the roadmap?"
        ));
    }

    #[test]
    fn follow_up_endpoint_uses_shorter_silence_than_idle_wake() {
        let silence = vec![0_i16; SAMPLES_PER_CHUNK];
        let speech = vec![3_000_i16; SAMPLES_PER_CHUNK];
        let mut gate = SpeechGate::calibrated_for_test();

        for _ in 0..2 {
            assert!(gate.push(&speech, true).is_none());
        }

        for _ in 0..required_end_silence_chunks(true, 2) - 1 {
            assert!(gate.push(&silence, true).is_none());
        }

        assert!(gate.push(&silence, true).is_some());
    }

    #[test]
    fn acknowledgement_chime_contains_audio() {
        let samples = acknowledgement_samples();
        assert_eq!(samples.len(), SAMPLE_RATE as usize * 120 / 1_000);
        assert!(samples.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn speech_gate_emits_after_bounded_trailing_silence() {
        let mut gate = SpeechGate::calibrated_for_test();
        let silence = vec![0_i16; SAMPLES_PER_CHUNK];
        let speech = vec![3_000_i16; SAMPLES_PER_CHUNK];

        for _ in 0..3 {
            assert!(gate.push(&silence, false).is_none());
        }
        for _ in 0..4 {
            assert!(gate.push(&speech, false).is_none());
        }

        let mut emitted = None;
        for _ in 0..WAKE_END_SILENCE_CHUNKS {
            emitted = gate.push(&silence, false).or(emitted);
        }

        assert!(emitted.is_some());
        assert!(emitted.expect("segment should exist").len() >= SAMPLES_PER_CHUNK * 4);
    }
}
