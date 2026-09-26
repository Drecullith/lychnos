use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use lychnos_core::speech::{
    SpeechSynthesisRequest, SpeechSynthesisResult, SpeechSynthesizer, SpeechVoiceProfile,
};

#[derive(Debug, Clone)]
pub struct PiperTts {
    binary_path: PathBuf,
    model_path: PathBuf,
    config_path: PathBuf,
}

impl PiperTts {
    pub fn discover() -> Result<Self, String> {
        let binary_path = env::var_os("LYCHNOS_PIPER_BIN")
            .map(PathBuf::from)
            .or_else(default_binary_path)
            .ok_or_else(|| {
                "Piper is not installed; run scripts/install-local-tts.sh".to_string()
            })?;

        let model_path = env::var_os("LYCHNOS_PIPER_MODEL")
            .map(PathBuf::from)
            .or_else(default_model_path)
            .ok_or_else(|| {
                "Lychnos TTS voice model is not installed; run scripts/install-local-tts.sh"
                    .to_string()
            })?;

        let config_path = env::var_os("LYCHNOS_PIPER_CONFIG")
            .map(PathBuf::from)
            .or_else(|| {
                let candidate = PathBuf::from(format!("{}.json", model_path.display()));
                candidate.is_file().then_some(candidate)
            })
            .ok_or_else(|| {
                "Lychnos TTS voice config is not installed; run scripts/install-local-tts.sh"
                    .to_string()
            })?;

        if !binary_path.is_file() {
            return Err(format!(
                "Piper binary not found at {}",
                binary_path.display()
            ));
        }
        if !model_path.is_file() {
            return Err(format!("Piper model not found at {}", model_path.display()));
        }
        if !config_path.is_file() {
            return Err(format!(
                "Piper config not found at {}",
                config_path.display()
            ));
        }

        Ok(Self {
            binary_path,
            model_path,
            config_path,
        })
    }

    pub fn description(&self) -> String {
        format!(
            "Piper · {}",
            self.model_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("voice")
        )
    }
}

impl SpeechSynthesizer for PiperTts {
    type Error = String;

    fn synthesize_to_path(
        &self,
        request: &SpeechSynthesisRequest,
        output_path: &Path,
    ) -> Result<SpeechSynthesisResult, Self::Error> {
        if request.is_empty() {
            return Err("refusing to synthesize empty speech".into());
        }

        let parent = output_path
            .parent()
            .ok_or_else(|| format!("output has no parent: {}", output_path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

        let speaking_rate = request.voice.speaking_rate.clamp(0.5, 2.0);
        let length_scale = 1.0 / speaking_rate;
        let volume = request.voice.volume.clamp(0.0, 2.0);

        let mut child = Command::new(&self.binary_path)
            .args([
                "-m",
                path_str(&self.model_path)?,
                "-c",
                path_str(&self.config_path)?,
                "-f",
                path_str(output_path)?,
                "--length-scale",
                &format!("{length_scale:.3}"),
                "--volume",
                &format!("{volume:.3}"),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start Piper: {error}"))?;

        {
            use std::io::Write;
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "Piper stdin was unavailable".to_string())?;
            writeln!(stdin, "{}", request.text)
                .map_err(|error| format!("failed to send text to Piper: {error}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("failed waiting for Piper: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "Piper exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let metadata =
            fs::metadata(output_path).map_err(|error| format!("Piper produced no WAV: {error}"))?;
        if metadata.len() <= 44 {
            return Err("Piper produced an empty WAV".into());
        }

        Ok(SpeechSynthesisResult {
            sample_rate_hz: Some(22_050),
            duration_ms: None,
        })
    }
}

pub struct SpeechOutputWorker {
    sender: Sender<String>,
    speaking: Arc<AtomicBool>,
}

impl SpeechOutputWorker {
    pub fn start(tts: PiperTts) -> Self {
        let (sender, receiver) = mpsc::channel::<String>();
        let speaking = Arc::new(AtomicBool::new(false));
        let worker_speaking = Arc::clone(&speaking);

        thread::Builder::new()
            .name("lychnos-speech-output".into())
            .spawn(move || {
                let voice = SpeechVoiceProfile::lychnos_default();

                for text in receiver {
                    worker_speaking.store(true, Ordering::Release);
                    let path = speech_output_path();
                    let request = SpeechSynthesisRequest::new(text, voice.clone());

                    match tts.synthesize_to_path(&request, &path) {
                        Ok(_) => {
                            let status = Command::new("pw-play").arg(&path).status();
                            if let Err(error) = status {
                                eprintln!("Speech playback failed: {error}");
                            }
                        }
                        Err(error) => {
                            eprintln!("Speech synthesis failed: {error}");
                        }
                    }

                    if let Err(error) = fs::remove_file(&path)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        eprintln!(
                            "Failed to delete synthesized speech {}: {error}",
                            path.display()
                        );
                    }

                    worker_speaking.store(false, Ordering::Release);
                }
            })
            .expect("Lychnos speech output worker should start");

        Self { sender, speaking }
    }

    #[must_use]
    pub fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::Acquire)
    }

    pub fn speak(&self, text: impl Into<String>) -> Result<(), String> {
        self.speaking.store(true, Ordering::Release);
        if self.sender.send(text.into()).is_err() {
            self.speaking.store(false, Ordering::Release);
            return Err("speech output worker is unavailable".to_string());
        }
        Ok(())
    }
}

fn speech_output_path() -> PathBuf {
    let root = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("lychnos/speech");

    root.join(format!(
        "reply-{}-{}.wav",
        std::process::id(),
        unique_nanos()
    ))
}

fn default_binary_path() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;
    let path = home.join(".local/lib/lychnos/tts-venv/bin/piper");
    path.is_file().then_some(path)
}

fn default_model_path() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;
    let path = home.join(".local/share/lychnos/voices/en_GB-alan-medium/en_GB-alan-medium.onnx");
    path.is_file().then_some(path)
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_explicit_piper_binary_fails_discovery() {
        let old_bin = env::var_os("LYCHNOS_PIPER_BIN");
        let old_model = env::var_os("LYCHNOS_PIPER_MODEL");
        let old_config = env::var_os("LYCHNOS_PIPER_CONFIG");

        unsafe {
            env::set_var("LYCHNOS_PIPER_BIN", "/definitely/missing/piper");
            env::set_var("LYCHNOS_PIPER_MODEL", "/definitely/missing/voice.onnx");
            env::set_var(
                "LYCHNOS_PIPER_CONFIG",
                "/definitely/missing/voice.onnx.json",
            );
        }

        assert!(PiperTts::discover().is_err());

        unsafe {
            restore_env("LYCHNOS_PIPER_BIN", old_bin);
            restore_env("LYCHNOS_PIPER_MODEL", old_model);
            restore_env("LYCHNOS_PIPER_CONFIG", old_config);
        }
    }

    unsafe fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => unsafe { env::set_var(key, value) },
            None => unsafe { env::remove_var(key) },
        }
    }
}
