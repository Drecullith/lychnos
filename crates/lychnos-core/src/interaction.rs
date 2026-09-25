//! Provider-neutral conversation contracts for Lychnos.
//!
//! Interaction requests and responses carry text and provenance only. They do
//! not grant system authority and do not couple Lychnos identity to one model.

use serde::{Deserialize, Serialize};

use crate::persona::{PersonaId, PersonaProfile};

/// Current wire schema for conversation requests and responses.
pub const INTERACTION_SCHEMA_VERSION: u32 = 1;

/// Opaque identifier for one user interaction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InteractionId(String);

impl InteractionId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How the user initiated an interaction.
///
/// Voice modes are defined now so push-to-talk and wake-word capture can feed
/// the same provider-neutral conversation path later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionSource {
    Typed,
    PushToTalk,
    WakeWord,
    VoiceSession,
}

/// One normalized user message entering the Lychnos interaction boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationRequest {
    pub id: InteractionId,
    pub source: InteractionSource,
    pub text: String,
}

impl ConversationRequest {
    #[must_use]
    pub fn new(id: InteractionId, source: InteractionSource, text: impl Into<String>) -> Self {
        Self {
            id,
            source,
            text: text.into(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// One provider-neutral response returned to a presentation surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationResponse {
    pub request_id: InteractionId,
    pub persona_id: PersonaId,
    pub persona_name: String,
    pub text: String,
}

impl ConversationResponse {
    #[must_use]
    pub fn new(
        request_id: InteractionId,
        persona: &PersonaProfile,
        text: impl Into<String>,
    ) -> Self {
        Self {
            request_id,
            persona_id: persona.id.clone(),
            persona_name: persona.display_name.clone(),
            text: text.into(),
        }
    }
}

/// Versioned request envelope used between presentation and runtime processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationRequestEnvelope {
    pub schema_version: u32,
    pub request: ConversationRequest,
}

impl ConversationRequestEnvelope {
    #[must_use]
    pub const fn new(request: ConversationRequest) -> Self {
        Self {
            schema_version: INTERACTION_SCHEMA_VERSION,
            request,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, InteractionEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| InteractionEnvelopeError::Parse(error.to_string()))?;
        validate_schema(envelope.schema_version)?;
        Ok(envelope)
    }
}

/// Versioned response envelope used between runtime and presentation processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationResponseEnvelope {
    pub schema_version: u32,
    pub response: ConversationResponse,
}

impl ConversationResponseEnvelope {
    #[must_use]
    pub const fn new(response: ConversationResponse) -> Self {
        Self {
            schema_version: INTERACTION_SCHEMA_VERSION,
            response,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, InteractionEnvelopeError> {
        let envelope: Self = serde_json::from_str(input)
            .map_err(|error| InteractionEnvelopeError::Parse(error.to_string()))?;
        validate_schema(envelope.schema_version)?;
        Ok(envelope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionEnvelopeError {
    Parse(String),
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

fn validate_schema(found: u32) -> Result<(), InteractionEnvelopeError> {
    if found == INTERACTION_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(InteractionEnvelopeError::UnsupportedSchemaVersion {
            found,
            supported: INTERACTION_SCHEMA_VERSION,
        })
    }
}

/// Replaceable provider boundary for turning normalized user text into a reply.
pub trait ConversationProvider {
    type Error;

    fn respond(
        &self,
        persona: &PersonaProfile,
        request: &ConversationRequest,
    ) -> Result<ConversationResponse, Self::Error>;
}

/// Deterministic local provider used to prove interaction plumbing before a
/// real AI provider is selected.
#[derive(Debug, Default, Clone, Copy)]
pub struct MockConversationProvider;

impl ConversationProvider for MockConversationProvider {
    type Error = std::convert::Infallible;

    fn respond(
        &self,
        persona: &PersonaProfile,
        request: &ConversationRequest,
    ) -> Result<ConversationResponse, Self::Error> {
        let normalized = request.text.trim();
        let lower = normalized.to_lowercase();

        let text = if lower == "hi" || lower == "hello" || lower == "hey" {
            "I'm here. Typed conversation is alive; the real brain plugs into this same boundary next."
                .to_string()
        } else if lower.contains("who are you") {
            format!(
                "I'm {} — your local-first companion. My identity belongs to Lychnos, not to whichever AI provider is connected.",
                persona.display_name
            )
        } else {
            format!(
                "Got it. I heard: “{normalized}”\n\nI'm still using the local mock conversation provider, but the Lychnos interaction path is working."
            )
        };

        Ok(ConversationResponse::new(request.id.clone(), persona, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_request_round_trips_through_versioned_envelope() {
        let request = ConversationRequest::new(
            InteractionId::new("interaction-1"),
            InteractionSource::Typed,
            "Hello Lychnos",
        );

        let json = ConversationRequestEnvelope::new(request.clone())
            .to_json()
            .expect("request should serialize");
        let decoded =
            ConversationRequestEnvelope::from_json(&json).expect("request should deserialize");

        assert_eq!(decoded.request, request);
    }

    #[test]
    fn unknown_interaction_schema_fails_closed() {
        let request = ConversationRequest::new(
            InteractionId::new("interaction-1"),
            InteractionSource::Typed,
            "Hello",
        );
        let mut envelope = ConversationRequestEnvelope::new(request);
        envelope.schema_version += 1;

        let json = envelope.to_json().expect("request should serialize");
        assert!(matches!(
            ConversationRequestEnvelope::from_json(&json),
            Err(InteractionEnvelopeError::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn mock_provider_uses_lychnos_persona_without_system_authority() {
        let persona = PersonaProfile::lychnos_default();
        let request = ConversationRequest::new(
            InteractionId::new("interaction-2"),
            InteractionSource::Typed,
            "Who are you?",
        );

        let response = MockConversationProvider
            .respond(&persona, &request)
            .expect("mock provider cannot fail");

        assert_eq!(response.request_id, request.id);
        assert_eq!(response.persona_id, persona.id);
        assert!(response.text.contains("local-first companion"));
        assert!(response.text.contains("AI provider"));
    }
}
