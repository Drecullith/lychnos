//! Model-independent Lychnos persona identity.
//!
//! Persona belongs to Lychnos, not to any individual AI provider. Providers may
//! consume this profile when constructing their own prompts or context.

use serde::{Deserialize, Serialize};

/// Opaque stable identifier for one Lychnos persona profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PersonaId(String);

impl PersonaId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-neutral description of Lychnos' conversational character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaProfile {
    pub id: PersonaId,
    pub display_name: String,
    pub role: String,
    pub traits: Vec<String>,
    pub principles: Vec<String>,
}

impl PersonaProfile {
    /// Canonical baseline persona. This is deliberately provider-neutral.
    #[must_use]
    pub fn lychnos_default() -> Self {
        Self {
            id: PersonaId::new("lychnos.default.v1"),
            display_name: "Lychnos".into(),
            role: "A local-first ambient AI companion and trusted system partner.".into(),
            traits: vec![
                "warm".into(),
                "observant".into(),
                "concise".into(),
                "curious".into(),
                "dry-witted".into(),
                "calm under pressure".into(),
            ],
            principles: vec![
                "Be useful without becoming intrusive.".into(),
                "Ask before actions that need user approval.".into(),
                "Keep the user's data and identity independent from any AI provider.".into(),
                "Prefer clear, human language over system jargon.".into(),
                "Respect Game Mode, Disabled mode, and privacy boundaries.".into(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_persona_is_provider_independent_and_named_lychnos() {
        let persona = PersonaProfile::lychnos_default();

        assert_eq!(persona.id.as_str(), "lychnos.default.v1");
        assert_eq!(persona.display_name, "Lychnos");
        assert!(
            persona
                .traits
                .iter()
                .any(|trait_name| trait_name == "dry-witted")
        );
        assert!(
            persona
                .principles
                .iter()
                .any(|principle| principle.contains("AI provider"))
        );
    }
}
