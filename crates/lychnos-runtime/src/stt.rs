use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use lychnos_core::speech::{SpeechToTextProvider, SpeechTranscript};

#[derive(Debug, Clone)]
pub struct WhisperCppStt {
    cli_path: PathBuf,
    model_path: PathBuf,
    library_path: Option<PathBuf>,
}

impl WhisperCppStt {
    pub fn discover() -> Result<Self, String> {
        let cli_path = env::var_os("LYCHNOS_WHISPER_CLI")
            .map(PathBuf::from)
            .or_else(default_cli_path)
            .ok_or_else(|| "whisper-cli not found; run scripts/install-local-stt.sh".to_string())?;

        let model_path = env::var_os("LYCHNOS_WHISPER_MODEL")
            .map(PathBuf::from)
            .or_else(default_model_path)
            .ok_or_else(|| {
                "Whisper model not found; run scripts/install-local-stt.sh".to_string()
            })?;

        if !cli_path.is_file() {
            return Err(format!("whisper-cli not found at {}", cli_path.display()));
        }
        if !model_path.is_file() {
            return Err(format!(
                "Whisper model not found at {}",
                model_path.display()
            ));
        }

        let library_path = cli_path.parent().and_then(|bin_dir| {
            if bin_dir.join("libwhisper.so").is_file() {
                Some(bin_dir.to_path_buf())
            } else {
                bin_dir
                    .parent()
                    .map(|root| root.join("lib"))
                    .filter(|path| path.is_dir())
            }
        });

        Ok(Self {
            cli_path,
            model_path,
            library_path,
        })
    }

    pub fn description(&self) -> String {
        format!(
            "whisper.cpp · {}",
            self.model_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("model")
        )
    }
}

impl SpeechToTextProvider for WhisperCppStt {
    type Error = String;

    fn transcribe(&self, audio_path: &Path) -> Result<SpeechTranscript, Self::Error> {
        if !audio_path.is_file() {
            return Err(format!("audio file not found: {}", audio_path.display()));
        }

        let mut command = Command::new(&self.cli_path);
        command.args([
            "-m",
            self.model_path
                .to_str()
                .ok_or_else(|| "Whisper model path is not valid UTF-8".to_string())?,
            "-f",
            audio_path
                .to_str()
                .ok_or_else(|| "audio path is not valid UTF-8".to_string())?,
            "-l",
            "auto",
            "-nt",
            "-np",
            "-sns",
        ]);

        if let Some(library_path) = &self.library_path {
            let existing = env::var_os("LD_LIBRARY_PATH").unwrap_or_default();
            let mut joined = library_path.as_os_str().to_os_string();
            if !existing.is_empty() {
                joined.push(":");
                joined.push(existing);
            }
            command.env("LD_LIBRARY_PATH", joined);
        }

        let output = command
            .output()
            .map_err(|error| format!("failed to run whisper-cli: {error}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "whisper-cli exited with {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        let text = String::from_utf8(output.stdout)
            .map_err(|error| format!("whisper-cli output was not UTF-8: {error}"))?;
        let normalized = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if looks_like_non_speech_annotation(&normalized) {
            return Ok(SpeechTranscript::new(""));
        }

        Ok(SpeechTranscript::new(normalized))
    }
}

fn looks_like_non_speech_annotation(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_lowercase();
    let wrapped = (lower.starts_with('[') && lower.ends_with(']'))
        || (lower.starts_with('(') && lower.ends_with(')'));

    if !wrapped {
        return false;
    }

    [
        "music",
        "upbeat music",
        "background music",
        "applause",
        "clapping",
        "laughter",
        "laughing",
        "silence",
        "inaudible",
        "noise",
        "background noise",
    ]
    .iter()
    .any(|label| lower.contains(label))
}

fn default_cli_path() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;

    [
        home.join(".local/lib/lychnos/stt/bin/whisper-cli"),
        home.join(".cache/lychnos/whisper.cpp/build/bin/whisper-cli"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn default_model_path() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;

    [
        home.join(".local/share/lychnos/models/ggml-base.bin"),
        home.join(".cache/lychnos/whisper.cpp/models/ggml-base.bin"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_non_speech_annotations_are_filtered() {
        assert!(looks_like_non_speech_annotation("(upbeat music)"));
        assert!(looks_like_non_speech_annotation("[Music]"));
        assert!(looks_like_non_speech_annotation("[applause]"));
        assert!(!looks_like_non_speech_annotation("I like music"));
        assert!(!looks_like_non_speech_annotation("How are you bro?"));
    }

    #[test]
    fn missing_explicit_binary_fails_discovery() {
        let old_cli = env::var_os("LYCHNOS_WHISPER_CLI");
        let old_model = env::var_os("LYCHNOS_WHISPER_MODEL");

        // SAFETY: this test is single-threaded with respect to these Lychnos-only
        // environment variables in this binary.
        unsafe {
            env::set_var("LYCHNOS_WHISPER_CLI", "/definitely/missing/whisper-cli");
            env::set_var("LYCHNOS_WHISPER_MODEL", "/definitely/missing/model.bin");
        }

        assert!(WhisperCppStt::discover().is_err());

        unsafe {
            match old_cli {
                Some(value) => env::set_var("LYCHNOS_WHISPER_CLI", value),
                None => env::remove_var("LYCHNOS_WHISPER_CLI"),
            }
            match old_model {
                Some(value) => env::set_var("LYCHNOS_WHISPER_MODEL", value),
                None => env::remove_var("LYCHNOS_WHISPER_MODEL"),
            }
        }
    }
}
