//! Platform-independent voice input vocabulary.
//!
//! Host-specific audio discovery and capture belong in adapters. The core owns
//! normalized device descriptions and activation semantics only.

use serde::{Deserialize, Serialize};

/// Opaque identifier for one normalized audio input device.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AudioInputDeviceId(String);

impl AudioInputDeviceId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider- and platform-neutral description of one microphone/input source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInputDevice {
    pub id: AudioInputDeviceId,
    pub display_name: String,
    pub backend: String,
    pub channel_count: Option<u32>,
    pub is_default: bool,
}

impl AudioInputDevice {
    #[must_use]
    pub fn new(
        id: AudioInputDeviceId,
        display_name: impl Into<String>,
        backend: impl Into<String>,
    ) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            backend: backend.into(),
            channel_count: None,
            is_default: false,
        }
    }

    #[must_use]
    pub const fn with_channel_count(mut self, channel_count: u32) -> Self {
        self.channel_count = Some(channel_count);
        self
    }

    #[must_use]
    pub const fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }
}

/// User-facing activation policy for microphone capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceActivationMode {
    PushToTalk,
    WakeWord,
    VoiceSession,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_audio_input_preserves_platform_identity_without_core_coupling() {
        let device = AudioInputDevice::new(
            AudioInputDeviceId::new("alsa_input.example"),
            "Example microphone",
            "pipewire",
        )
        .with_channel_count(2)
        .with_default(true);

        assert_eq!(device.id.as_str(), "alsa_input.example");
        assert_eq!(device.display_name, "Example microphone");
        assert_eq!(device.backend, "pipewire");
        assert_eq!(device.channel_count, Some(2));
        assert!(device.is_default);
    }

    #[test]
    fn voice_activation_modes_cover_planned_desktop_paths() {
        assert_ne!(
            VoiceActivationMode::PushToTalk,
            VoiceActivationMode::WakeWord
        );
        assert_ne!(
            VoiceActivationMode::WakeWord,
            VoiceActivationMode::VoiceSession
        );
    }
}
