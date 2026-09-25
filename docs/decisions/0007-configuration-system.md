# ADR-0007: Initial Configuration System

Status: Accepted

## Context

Lychnos requires configuration without hard-coding runtime behavior.

Configuration must remain understandable, local-first, testable without Omarchy, and independent from future UI or service integration.

Secrets must not be mixed into ordinary configuration.

## Decision

Lychnos uses a versioned TOML configuration model.

The initial configuration schema contains:

- runtime startup mode
- memory enable/disable state
- audit enable/disable state
- cloud-request privacy policy

Built-in defaults are privacy-preserving:

- startup mode is Normal
- memory is enabled
- audit is enabled
- cloud requests are disabled

Unknown configuration fields are rejected to catch misspellings and unsupported settings.

The initial implementation parses configuration from TOML text but does not yet decide:

- filesystem location
- environment-variable overrides
- CLI overrides
- live reload
- secrets storage
- Omarchy-specific configuration discovery

Those belong to later integration work.

## Consequences

Lychnos gains a typed configuration model that can be tested independently of the operating system.

Configuration mistakes fail explicitly rather than being silently ignored.

Future loaders may layer files, environment variables, command-line arguments, or UI settings onto the same domain model.
