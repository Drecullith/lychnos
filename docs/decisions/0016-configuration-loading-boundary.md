# ADR-0016: Configuration Loading Boundary

Status: Accepted

## Context

Lychnos already has a typed, versioned TOML configuration schema with validation.

Before this decision, callers could parse TOML directly through `LychnosConfig::from_toml(...)`, but there was no explicit boundary separating:

- where configuration comes from
- how raw configuration is obtained
- how configuration is parsed and validated
- when validated defaults should be used

Without that separation, future file-system paths, environment-specific locations, packaged defaults, or other configuration sources could leak source-specific behavior into the schema layer.

## Decision

The machine-independent core defines a generic `ConfigSource` trait.

A configuration source returns:

- `Some(String)` when explicit TOML configuration is available
- `None` when no explicit configuration was supplied
- a source-specific error when retrieval fails

The core also provides a `ConfigLoader`.

`ConfigLoader`:

1. asks the configured source for optional raw TOML
2. parses and validates explicit TOML through the existing `LychnosConfig` schema
3. falls back to validated `LychnosConfig::default()` when the source is empty
4. preserves the distinction between source failures and configuration parse/validation failures

The loading boundary is represented by `ConfigLoadError<E>` with separate variants for:

- source errors
- configuration errors

The core deliberately does not decide:

- a permanent configuration file path
- platform-specific config directories
- environment-variable overrides
- CLI override precedence
- live reload behavior
- secrets storage

Those remain later adapter/application decisions.

## Consequences

Configuration acquisition is now replaceable without changing the configuration schema.

Tests and simulations can provide deterministic in-memory sources.

Future Omarchy, Linux, portable-device, or other platform adapters can implement `ConfigSource` while reusing the same validation boundary.

Missing configuration is explicitly different from failed configuration retrieval.

The default configuration remains local-first and validated through the same invariant checks used for explicit configuration.
