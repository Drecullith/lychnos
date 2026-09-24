//! Lychnos configuration types.

use serde::Deserialize;

use crate::runtime::RuntimeMode;

/// Current supported Lychnos configuration schema.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// Complete machine-independent Lychnos configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LychnosConfig {
    pub schema_version: u32,
    pub runtime: RuntimeConfig,
    pub memory: MemoryConfig,
    pub audit: AuditConfig,
    pub privacy: PrivacyConfig,
}

impl Default for LychnosConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            runtime: RuntimeConfig::default(),
            memory: MemoryConfig::default(),
            audit: AuditConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

impl LychnosConfig {
    /// Parses and validates Lychnos configuration from TOML text.
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        let config: Self =
            toml::from_str(input).map_err(|error| ConfigError::Parse(error.to_string()))?;

        config.validate()?;

        Ok(config)
    }

    /// Validates configuration invariants.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: CONFIG_SCHEMA_VERSION,
            });
        }

        Ok(())
    }
}

/// Runtime-related configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub startup_mode: StartupMode,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            startup_mode: StartupMode::Normal,
        }
    }
}

/// Runtime mode requested when Lychnos starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartupMode {
    #[default]
    Normal,
    GameMode,
    Disabled,
}

impl From<StartupMode> for RuntimeMode {
    fn from(value: StartupMode) -> Self {
        match value {
            StartupMode::Normal => Self::Normal,
            StartupMode::GameMode => Self::GameMode,
            StartupMode::Disabled => Self::Disabled,
        }
    }
}

/// Memory subsystem configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryConfig {
    pub enabled: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Security audit configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    pub enabled: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Privacy-related configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct PrivacyConfig {
    /// Whether Lychnos may make requests to cloud AI providers.
    ///
    /// This does not itself authorize any particular data to leave the machine.
    pub cloud_requests_enabled: bool,
}

/// Configuration loading or validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The TOML input could not be parsed into the Lychnos schema.
    Parse(String),

    /// The configuration schema version is unsupported.
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_local_first() {
        let config = LychnosConfig::default();

        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.runtime.startup_mode, StartupMode::Normal);
        assert!(config.memory.enabled);
        assert!(config.audit.enabled);
        assert!(!config.privacy.cloud_requests_enabled);
    }

    #[test]
    fn toml_can_override_supported_settings() {
        let config = LychnosConfig::from_toml(
            r#"
schema_version = 1

[runtime]
startup_mode = "game_mode"

[memory]
enabled = false

[audit]
enabled = true

[privacy]
cloud_requests_enabled = true
"#,
        )
        .expect("valid Lychnos configuration should parse");

        assert_eq!(config.runtime.startup_mode, StartupMode::GameMode);
        assert!(!config.memory.enabled);
        assert!(config.audit.enabled);
        assert!(config.privacy.cloud_requests_enabled);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let error = LychnosConfig::from_toml(
            r#"
schema_version = 1
mystery_setting = true
"#,
        )
        .expect_err("unknown configuration fields should fail");

        assert!(matches!(error, ConfigError::Parse(_)));
    }

    #[test]
    fn unsupported_schema_versions_are_rejected() {
        let error = LychnosConfig::from_toml(
            r#"
schema_version = 999
"#,
        )
        .expect_err("unsupported schema versions should fail");

        assert_eq!(
            error,
            ConfigError::UnsupportedSchemaVersion {
                found: 999,
                supported: CONFIG_SCHEMA_VERSION,
            }
        );
    }

    #[test]
    fn startup_mode_maps_to_runtime_mode() {
        assert_eq!(RuntimeMode::from(StartupMode::Normal), RuntimeMode::Normal);

        assert_eq!(
            RuntimeMode::from(StartupMode::GameMode),
            RuntimeMode::GameMode
        );

        assert_eq!(
            RuntimeMode::from(StartupMode::Disabled),
            RuntimeMode::Disabled
        );
    }
}
