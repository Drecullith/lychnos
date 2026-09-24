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

/// Source from which raw Lychnos configuration text can be loaded.
///
/// The core deliberately does not decide where configuration lives on disk.
/// Platform and application adapters can implement this boundary later.
pub trait ConfigSource {
    type Error;

    /// Loads optional TOML configuration text.
    ///
    /// Returning `None` means no explicit configuration was supplied and
    /// Lychnos should use its validated defaults.
    fn load(&self) -> Result<Option<String>, Self::Error>;
}

/// Failure while loading or validating configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadError<E> {
    /// The configuration source itself failed.
    Source(E),

    /// Configuration text was loaded but could not be parsed or validated.
    Config(ConfigError),
}

/// Machine-independent configuration loading boundary.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConfigLoader;

impl ConfigLoader {
    /// Loads configuration from a source, falling back to validated defaults
    /// when the source contains no explicit configuration.
    pub fn load<S: ConfigSource>(
        self,
        source: &S,
    ) -> Result<LychnosConfig, ConfigLoadError<S::Error>> {
        let input = source.load().map_err(ConfigLoadError::Source)?;

        match input {
            Some(input) => LychnosConfig::from_toml(&input).map_err(ConfigLoadError::Config),
            None => {
                let config = LychnosConfig::default();

                config.validate().map_err(ConfigLoadError::Config)?;

                Ok(config)
            }
        }
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

    #[derive(Debug)]
    struct StaticConfigSource {
        input: Option<String>,
    }

    impl StaticConfigSource {
        fn with_toml(input: &str) -> Self {
            Self {
                input: Some(input.into()),
            }
        }

        fn empty() -> Self {
            Self { input: None }
        }
    }

    impl ConfigSource for StaticConfigSource {
        type Error = &'static str;

        fn load(&self) -> Result<Option<String>, Self::Error> {
            Ok(self.input.clone())
        }
    }

    #[derive(Debug)]
    struct FailingConfigSource;

    impl ConfigSource for FailingConfigSource {
        type Error = &'static str;

        fn load(&self) -> Result<Option<String>, Self::Error> {
            Err("source unavailable")
        }
    }

    #[test]
    fn config_loader_uses_defaults_when_source_is_empty() {
        let config = ConfigLoader
            .load(&StaticConfigSource::empty())
            .expect("empty source should use defaults");

        assert_eq!(config, LychnosConfig::default());
    }

    #[test]
    fn config_loader_parses_explicit_source() {
        let source = StaticConfigSource::with_toml(
            r#"
schema_version = 1

[runtime]
startup_mode = "disabled"

[privacy]
cloud_requests_enabled = true
"#,
        );

        let config = ConfigLoader
            .load(&source)
            .expect("valid source should load");

        assert_eq!(config.runtime.startup_mode, StartupMode::Disabled);
        assert!(config.privacy.cloud_requests_enabled);
    }

    #[test]
    fn config_loader_preserves_config_errors() {
        let source = StaticConfigSource::with_toml(
            r#"
schema_version = 999
"#,
        );

        let error = ConfigLoader
            .load(&source)
            .expect_err("unsupported schema must fail");

        assert_eq!(
            error,
            ConfigLoadError::Config(ConfigError::UnsupportedSchemaVersion {
                found: 999,
                supported: CONFIG_SCHEMA_VERSION,
            })
        );
    }

    #[test]
    fn config_loader_preserves_source_errors() {
        let error = ConfigLoader
            .load(&FailingConfigSource)
            .expect_err("source failure must propagate");

        assert_eq!(error, ConfigLoadError::Source("source unavailable"));
    }

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
