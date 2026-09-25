//! Platform-independent voice input vocabulary.
//!
//! Host-specific audio discovery and capture belong in adapters. The core owns
//! normalized device descriptions and activation semantics only.

use serde::{Deserialize, Serialize};

use crate::interaction::InteractionId;

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

/// Opaque identifier binding one explicit microphone-capture session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VoiceCaptureId(String);

impl VoiceCaptureId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Explicit user control for microphone capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceCaptureCommand {
    StartPushToTalk,
    StopPushToTalk,
}

/// Authority-free request from a presentation surface to the voice adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCaptureRequest {
    pub capture_id: VoiceCaptureId,
    pub command: VoiceCaptureCommand,
    pub device_id: Option<AudioInputDeviceId>,
}

/// Current schema for voice-capture control messages.
pub const VOICE_CONTROL_SCHEMA_VERSION: u32 = 1;

/// Versioned voice-capture control envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCaptureControlEnvelope {
    pub schema_version: u32,
    pub request: VoiceCaptureRequest,
}

impl VoiceCaptureControlEnvelope {
    #[must_use]
    pub const fn new(request: VoiceCaptureRequest) -> Self {
        Self {
            schema_version: VOICE_CONTROL_SCHEMA_VERSION,
            request,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, VoiceControlEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| VoiceControlEnvelopeError::Parse(error.to_string()))?;

        if envelope.schema_version != VOICE_CONTROL_SCHEMA_VERSION {
            return Err(VoiceControlEnvelopeError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                supported: VOICE_CONTROL_SCHEMA_VERSION,
            });
        }

        Ok(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceControlEnvelopeError {
    Parse(String),
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

/// Runtime acknowledgement state for one explicit voice capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceCaptureState {
    Started,
    Stopped,
    Transcribing,
    Transcribed,
    Failed,
    TimedOut,
}

/// Runtime-owned status projected back to presentation surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCaptureStatus {
    pub capture_id: VoiceCaptureId,
    pub state: VoiceCaptureState,
    pub captured_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub transcript: Option<String>,
    pub interaction_id: Option<InteractionId>,
    pub detail: String,
}

/// Versioned runtime-to-presentation voice status envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceCaptureStatusEnvelope {
    pub schema_version: u32,
    pub status: VoiceCaptureStatus,
}

impl VoiceCaptureStatusEnvelope {
    #[must_use]
    pub const fn new(status: VoiceCaptureStatus) -> Self {
        Self {
            schema_version: VOICE_CONTROL_SCHEMA_VERSION,
            status,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, VoiceControlEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| VoiceControlEnvelopeError::Parse(error.to_string()))?;

        if envelope.schema_version != VOICE_CONTROL_SCHEMA_VERSION {
            return Err(VoiceControlEnvelopeError::UnsupportedSchemaVersion {
                found: envelope.schema_version,
                supported: VOICE_CONTROL_SCHEMA_VERSION,
            });
        }

        Ok(envelope)
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
    fn push_to_talk_control_round_trips_with_capture_identity() {
        let request = VoiceCaptureRequest {
            capture_id: VoiceCaptureId::new("capture-1"),
            command: VoiceCaptureCommand::StartPushToTalk,
            device_id: Some(AudioInputDeviceId::new("alsa_input.example")),
        };
        let json = VoiceCaptureControlEnvelope::new(request.clone())
            .to_json()
            .expect("voice control should serialize");
        let decoded =
            VoiceCaptureControlEnvelope::from_json(&json).expect("voice control should decode");

        assert_eq!(decoded.request, request);
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
