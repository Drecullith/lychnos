//! Provider-independent Lychnos intelligence routing.
//!
//! Lychnos owns identity, memory, persona, permissions, and initiative. Model
//! runtimes are replaceable capabilities selected according to the current body.

use serde::{Deserialize, Serialize};

/// Broad body/platform class used for capability-aware defaults.
///
/// This is descriptive metadata only. Platform adapters own actual hardware
/// probing and inference-runtime selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyPlatform {
    LinuxDesktop,
    WindowsDesktop,
    MacDesktop,
    Android,
    Ios,
    PortableEmbedded,
    Other,
}

/// Coarse local-brain size class.
///
/// These classes describe expected resource envelopes, not specific model names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalBrainClass {
    Tiny,
    Compact,
    Standard,
    Large,
}

/// Normalized hardware capability snapshot supplied by a platform adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCapabilityProfile {
    pub platform: BodyPlatform,
    pub memory_mebibytes: u64,
    pub logical_cpu_count: u16,
    pub accelerator_available: bool,
    pub low_power_body: bool,
}

impl DeviceCapabilityProfile {
    #[must_use]
    pub const fn new(
        platform: BodyPlatform,
        memory_mebibytes: u64,
        logical_cpu_count: u16,
        accelerator_available: bool,
        low_power_body: bool,
    ) -> Self {
        Self {
            platform,
            memory_mebibytes,
            logical_cpu_count,
            accelerator_available,
            low_power_body,
        }
    }
}

/// Portable metadata describing one installable local model.
///
/// Runtime-specific details such as GGUF, Core ML, ONNX, or another format stay
/// outside this core type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalModelManifest {
    pub id: String,
    pub display_name: String,
    pub class: LocalBrainClass,
    pub minimum_memory_mebibytes: u64,
    pub preferred_memory_mebibytes: u64,
    pub context_tokens: u32,
    pub license: String,
}

impl LocalModelManifest {
    #[must_use]
    pub fn fits(&self, capabilities: &DeviceCapabilityProfile) -> bool {
        capabilities.memory_mebibytes >= self.minimum_memory_mebibytes
    }
}

/// Baseline policy for choosing a local-brain resource class.
///
/// The selected class is intentionally conservative; a platform adapter or user
/// may choose a different compatible model later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LocalBrainSelectionPolicy;

impl LocalBrainSelectionPolicy {
    #[must_use]
    pub fn recommended_class(&self, capabilities: &DeviceCapabilityProfile) -> LocalBrainClass {
        if capabilities.low_power_body || capabilities.memory_mebibytes < 6 * 1024 {
            LocalBrainClass::Tiny
        } else if capabilities.memory_mebibytes < 12 * 1024 {
            LocalBrainClass::Compact
        } else if capabilities.memory_mebibytes < 32 * 1024 {
            LocalBrainClass::Standard
        } else {
            LocalBrainClass::Large
        }
    }

    #[must_use]
    pub fn select<'a>(
        &self,
        capabilities: &DeviceCapabilityProfile,
        models: &'a [LocalModelManifest],
    ) -> Option<&'a LocalModelManifest> {
        let recommended = self.recommended_class(capabilities);

        models
            .iter()
            .filter(|model| model.fits(capabilities))
            .filter(|model| model.class <= recommended)
            .max_by_key(|model| {
                (
                    model.class,
                    model.preferred_memory_mebibytes <= capabilities.memory_mebibytes,
                    model.context_tokens,
                )
            })
            .or_else(|| {
                models
                    .iter()
                    .filter(|model| model.fits(capabilities))
                    .min_by_key(|model| model.minimum_memory_mebibytes)
            })
    }
}

/// High-level intelligence source. Local remains the no-subscription baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntelligenceSource {
    Local,
    AccountBridge { provider: String },
    ApiByok { provider: String },
}

/// User-owned routing preference.
///
/// Account bridges and API keys are optional additions; local intelligence
/// remains a valid route even when no external provider is configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntelligenceRoutingPreferences {
    pub allow_local: bool,
    pub preferred_account_bridge: Option<String>,
    pub allow_api_byok: bool,
}

impl Default for IntelligenceRoutingPreferences {
    fn default() -> Self {
        Self {
            allow_local: true,
            preferred_account_bridge: None,
            allow_api_byok: false,
        }
    }
}

/// Provider-neutral router decision.
///
/// This does not perform authentication or inference; adapters own those tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntelligenceRouter;

impl IntelligenceRouter {
    #[must_use]
    pub fn choose_source(
        &self,
        preferences: &IntelligenceRoutingPreferences,
        connected_account_bridges: &[String],
    ) -> Option<IntelligenceSource> {
        if let Some(preferred) = preferences.preferred_account_bridge.as_ref()
            && connected_account_bridges
                .iter()
                .any(|provider| provider == preferred)
        {
            return Some(IntelligenceSource::AccountBridge {
                provider: preferred.clone(),
            });
        }

        if preferences.allow_local {
            return Some(IntelligenceSource::Local);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(
        id: &str,
        class: LocalBrainClass,
        minimum_memory_mebibytes: u64,
    ) -> LocalModelManifest {
        LocalModelManifest {
            id: id.into(),
            display_name: id.into(),
            class,
            minimum_memory_mebibytes,
            preferred_memory_mebibytes: minimum_memory_mebibytes,
            context_tokens: 4096,
            license: "test".into(),
        }
    }

    #[test]
    fn low_power_portable_defaults_to_tiny_class() {
        let capabilities =
            DeviceCapabilityProfile::new(BodyPlatform::PortableEmbedded, 8_192, 8, false, true);

        assert_eq!(
            LocalBrainSelectionPolicy.recommended_class(&capabilities),
            LocalBrainClass::Tiny
        );
    }

    #[test]
    fn phone_scale_memory_prefers_compact_over_desktop_large() {
        let capabilities =
            DeviceCapabilityProfile::new(BodyPlatform::Android, 8_192, 8, true, false);
        let models = vec![
            model("tiny", LocalBrainClass::Tiny, 2_048),
            model("compact", LocalBrainClass::Compact, 5_120),
            model("large", LocalBrainClass::Large, 24_576),
        ];

        assert_eq!(
            LocalBrainSelectionPolicy
                .select(&capabilities, &models)
                .expect("a phone-compatible model should be selected")
                .id,
            "compact"
        );
    }

    #[test]
    fn large_desktop_can_select_large_class() {
        let capabilities =
            DeviceCapabilityProfile::new(BodyPlatform::LinuxDesktop, 61_440, 24, true, false);
        let models = vec![
            model("compact", LocalBrainClass::Compact, 5_120),
            model("standard", LocalBrainClass::Standard, 10_240),
            model("large", LocalBrainClass::Large, 24_576),
        ];

        assert_eq!(
            LocalBrainSelectionPolicy
                .select(&capabilities, &models)
                .expect("desktop model should be selected")
                .id,
            "large"
        );
    }

    #[test]
    fn local_is_the_default_no_subscription_route() {
        assert_eq!(
            IntelligenceRouter.choose_source(&IntelligenceRoutingPreferences::default(), &[]),
            Some(IntelligenceSource::Local)
        );
    }

    #[test]
    fn connected_preferred_account_bridge_can_override_local_route() {
        let preferences = IntelligenceRoutingPreferences {
            allow_local: true,
            preferred_account_bridge: Some("codex".into()),
            allow_api_byok: false,
        };

        assert_eq!(
            IntelligenceRouter
                .choose_source(&preferences, &["codex".into(), "another-provider".into()]),
            Some(IntelligenceSource::AccountBridge {
                provider: "codex".into()
            })
        );
    }

    #[test]
    fn missing_preferred_bridge_falls_back_to_local() {
        let preferences = IntelligenceRoutingPreferences {
            allow_local: true,
            preferred_account_bridge: Some("codex".into()),
            allow_api_byok: false,
        };

        assert_eq!(
            IntelligenceRouter.choose_source(&preferences, &[]),
            Some(IntelligenceSource::Local)
        );
    }
}
