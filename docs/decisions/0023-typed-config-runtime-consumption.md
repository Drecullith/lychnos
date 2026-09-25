# ADR-0023: Typed Configuration Consumption by the Runtime

Status: Accepted

## Context

Lychnos already had a typed, versioned configuration schema and a machine-independent configuration-loading boundary.

Until Phase 2, that configuration was still largely descriptive: the foundation runtime accepted a `RuntimeMode` directly, and the CLI translated its arguments straight into runtime state.

That meant the typed configuration model existed, but it was not yet the authoritative startup input for the running prototype.

## Decision

`FoundationRuntime` now provides:

```text
FoundationRuntime::from_config(...)
```

This constructor accepts a validated `LychnosConfig` and derives the live startup runtime mode from:

```text
config.runtime.startup_mode
```

The configured startup mode therefore controls whether the runtime begins in:

- Normal
- Game Mode
- Disabled

The CLI now constructs a `LychnosConfig` and passes that typed configuration into the foundation runtime rather than constructing `RuntimeMode` directly.

The CLI's existing simulation arguments currently act as a temporary adapter that modifies the typed config before runtime creation.

Other configuration sections are not consumed prematurely. Memory, diagnostics, privacy, persistence, and provider behavior will each be wired into the subsystem that owns that behavior when the corresponding runtime boundary exists.

## Consequences

The typed configuration model is now part of the actual prototype startup path.

Runtime startup behavior can be tested through the same configuration object future application and platform adapters will use.

The runtime remains independent of filesystem layout and host-specific configuration discovery.

## Deliberately Deferred

This decision does not choose:

- a permanent configuration file path
- XDG or Omarchy-specific configuration discovery
- environment-variable precedence
- command-line override precedence
- secrets storage
- live configuration reload
- configuration persistence

Those are application or platform-adapter concerns and should not leak into the machine-independent core prematurely.

## Current Limitations

Only `runtime.startup_mode` is currently consumed by `FoundationRuntime::from_config(...)`.

The CLI still uses simple command-line arguments as a temporary way to construct the typed configuration used by the simulation.

No real host configuration file is read yet.
