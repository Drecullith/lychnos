//! Provider-neutral speech input/output contracts.
//!
//! Host-specific capture, recognition engines, synthesis engines, and playback
//! belong to adapters. PTT, wake-word, typed chat, and future presentation
//! surfaces can therefore share one Lychnos-owned speech vocabulary.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Stable Lychnos-owned identifier for one synthesized voice profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SpeechVoiceId(String);

impl SpeechVoiceId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-neutral description of how Lychnos should sound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechVoiceProfile {
    pub id: SpeechVoiceId,
    pub display_name: String,
    pub language: String,
    pub speaking_rate: f32,
    pub volume: f32,
}

impl SpeechVoiceProfile {
    #[must_use]
    pub fn lychnos_default() -> Self {
        Self {
            id: SpeechVoiceId::new("lychnos.voice.default.v1"),
            display_name: "Lychnos".into(),
            language: "en-GB".into(),
            speaking_rate: 1.0,
            volume: 1.0,
        }
    }
}

/// One request to synthesize Lychnos speech.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechSynthesisRequest {
    pub text: String,
    pub voice: SpeechVoiceProfile,
}

impl SpeechSynthesisRequest {
    #[must_use]
    pub fn new(text: impl Into<String>, voice: SpeechVoiceProfile) -> Self {
        Self {
            text: text.into(),
            voice,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Result metadata from a successful synthesis operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechSynthesisResult {
    pub sample_rate_hz: Option<u32>,
    pub duration_ms: Option<u64>,
}

/// Replaceable boundary for local or remote speech synthesis.
pub trait SpeechSynthesizer {
    type Error;

    fn synthesize_to_path(
        &self,
        request: &SpeechSynthesisRequest,
        output_path: &Path,
    ) -> Result<SpeechSynthesisResult, Self::Error>;
}

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
    fn canonical_voice_profile_is_lychnos_owned() {
        let voice = SpeechVoiceProfile::lychnos_default();
        assert_eq!(voice.id.as_str(), "lychnos.voice.default.v1");
        assert_eq!(voice.display_name, "Lychnos");
        assert_eq!(voice.language, "en-GB");
    }

    #[test]
    fn synthesis_request_empty_check_ignores_whitespace() {
        let request = SpeechSynthesisRequest::new("   ", SpeechVoiceProfile::lychnos_default());
        assert!(request.is_empty());
    }

    #[test]
    fn transcript_empty_check_ignores_whitespace() {
        assert!(SpeechTranscript::new("   ").is_empty());
        assert!(!SpeechTranscript::new("hello").is_empty());
    }
}
