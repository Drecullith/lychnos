//! Provider-neutral speech-to-text contracts.
//!
//! Host-specific capture belongs to voice adapters. Speech recognition is a
//! replaceable provider boundary so PTT, wake-word, and voice-session input can
//! share one transcription path.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Normalized result from a speech-to-text provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechTranscript {
    pub text: String,
    pub language: Option<String>,
}

impl SpeechTranscript {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Replaceable boundary for local or remote speech recognition.
pub trait SpeechToTextProvider {
    type Error;

    fn transcribe(&self, audio_path: &Path) -> Result<SpeechTranscript, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_empty_check_ignores_whitespace() {
        assert!(SpeechTranscript::new("   ").is_empty());
        assert!(!SpeechTranscript::new("hello").is_empty());
    }
}
