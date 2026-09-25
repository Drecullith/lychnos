# Project Lychnos Architecture

## Status

This document describes the current architectural baseline of Project Lychnos.

Lychnos is still in its machine-independent foundation phase. Real Omarchy, Linux service, hardware, voice, AI-provider, and privileged-execution integrations are intentionally not implemented yet.

## Architectural Principles

Lychnos is:

- local-first
- open-source
- built around a platform-independent Rust core
- Omarchy-first, but not Omarchy-dependent
- model-independent at the identity and memory layers
- permission-mediated for meaningful actions
- auditable
- designed for immediate disable
- designed for low-impact Game Mode operation
- built incrementally with testable boundaries

## High-Level Architecture

```text
Platform / Input Adapters
          |
          v
       Collector
          |
          v
   Normalized Event
          |
          v
    Internal Event Bus
          |
          v
       Analyzer
          |
          v
   Action Proposal
          |
          v
 Live Runtime Safety
          |
          v
  Permission Policy
          |
          v
 Execution Boundary
          |
          v
      Audit Trail
```

Configuration, diagnostics, and Lychnos-owned memory exist alongside this processing path. Security audit remains part of the security-critical processing boundary.

## Core Modules

The platform-independent foundation currently lives in `crates/lychnos-core`.

### event

Defines the normalized event envelope used inside Lychnos.

Platform-specific collectors must translate native input into this format before entering the core.

### collector

Defines the machine-independent event-source boundary.

Collectors produce already-normalized `Event` values before they enter the core processing path.

The current `MockCollector` is deterministic and in-memory. `FoundationRuntime::collect_once(...)` suppresses collector polling whenever background work is not allowed, so Game Mode and Disabled prevent collectors from being polled.

### bus

Provides the current in-process publish/subscribe event bus.

The current implementation uses the Rust standard library and is intentionally replaceable.

### analyzer

Contains the deterministic `MockAnalyzer` used to prove the architecture.

It is not the eventual AI reasoning layer.

### action

Defines structured action proposals, capabilities, parameters, impact classifications, and risk classifications.

AI providers and analyzers must propose structured actions rather than execute arbitrary system commands.

### permission

Defines the authorization boundary.

The default policy consults the live `RuntimeController`.

Current behavior:

```text
Normal + read-only       -> allowed
Normal + state-changing  -> user approval required
Normal + external        -> user approval required
Normal + privileged      -> user approval required
Normal + destructive     -> user approval required
Game Mode                -> denied
Disabled                 -> denied
```

### runtime

Defines Lychnos runtime safety state.

Current modes:

```text
Normal
GameMode
Disabled
```

The `RuntimeController` is thread-safe.

Disabled mode fails closed and cannot be weakened by ordinary Game Mode transitions. Leaving Disabled requires an explicit enable operation.

Already-started simulated work receives a runtime lease. Game Mode exposes a reversible pause request, while Disabled permanently invalidates older leases and exposes a cancellation request that cannot be cleared by later re-enable.

### executor

Contains the foundation-phase `MockExecutor`.

It does not execute anything.

It proves the authorization boundary by returning one of:

```text
WouldExecute
AwaitingUserApproval
Blocked
```

The executor evaluates live runtime state itself instead of trusting a caller-provided permission result.

The same module contains simulation-only running-work lifecycle types. They distinguish pause/cancellation requests from acknowledged pause, cancellation-unavailable, normal completion, and confirmed stop-after-cancellation.

Deterministic cooperation-window assessment accepts caller-supplied elapsed time and timeout budgets. It does not choose a wall-clock timer, async runtime, or escalation mechanism.

### audit

Defines structured security audit records and the `AuditSink` interface.

The security audit trail is mandatory and cannot be disabled through ordinary configuration.

The current `InMemoryAuditLog` is append-only through its public API.

Persistent, tamper-resistant audit storage is future work.

Mock running-work transitions use a dedicated `SimulationWorkLifecycleChanged` event with an explicit simulation marker. Reserved real-execution audit kinds are not used to imply that host execution occurred.

### diagnostics

Defines ordinary troubleshooting and development records separately from the security audit system.

The current `InMemoryDiagnosticLog` is a foundation implementation behind the `DiagnosticSink` interface.

Diagnostics may be disabled independently. They do not replace mandatory security audit records.

### resource

Defines machine-independent resource observations and mode-specific resource budgets for later performance coexistence measurement.

The current module provides typed metric names, explicit units, deterministic budget evaluation, a `ResourceSink` interface, and an in-memory observation log.

It does not sample the host. Linux/Omarchy telemetry sources, real thresholds, sampling cadence, measurement windows, and background scheduling remain adapter-level future work.

### presentation

Defines owned read-only state intended for desktop and portable presentation layers.

The current `CompanionPresentationState` projects runtime mode, pending approvals, tracked simulation work, and the latest ordinary diagnostic without exposing approval grants, executors, runtime controllers, or mutation handles.

A versioned `CompanionPresentationEnvelope` provides a serializable read-only wire contract. The current Phase 2 runtime publisher atomically writes this projection to a session-local snapshot, and presentation processes may consume that snapshot without gaining runtime authority.

This allows an early visual shell to follow live core state without becoming an execution authority.

### visual shell prototype

`prototypes/lychnos-shell` is an isolated Omarchy/Wayland experiment built with GTK4 and gtk4-layer-shell.

It is deliberately outside the main Cargo workspace so Linux desktop native dependencies do not become requirements for the platform-independent core or its standard CI.

The shell uses presentation-domain data only, requests no keyboard interactivity, reserves no screen space, and renders the validated canonical Lychnos body asset with state-driven cyan expressions plus a compact status card.

It now follows the versioned read-only presentation snapshot produced by a separately owned `FoundationRuntime`. The current JSON/session-file transport is intentionally replaceable and is not a final IPC decision.

This prototype is not a final UI-toolkit decision.

### memory

Defines Lychnos-owned, model-independent memory records and the `MemoryStore` interface.

The current implementation is an in-memory store.

The authoritative memory model belongs to Lychnos rather than any AI provider.

### config

Defines typed, versioned TOML configuration.

The current configuration schema is version 2.

Current defaults are local-first:

```text
runtime mode:       Normal
memory:             enabled
diagnostics:        enabled
cloud AI requests:  disabled
security audit:     mandatory
```

Security audit has no configuration disable switch.

Unknown configuration fields and legacy schema version 1 are rejected.

### orchestrator

Defines the current machine-independent `FoundationRuntime`.

It owns the foundation event bus, internal subscription, runtime safety controller, in-memory security audit log, and injected ID/time providers.

It accepts already-normalized events directly and can also pull one event from a collector while respecting runtime background-work suppression.

For Phase 2 simulation it also retains mock running-work records by action ID, exposes read-only lifecycle snapshots, mediates audited lifecycle transitions, and produces an owned companion presentation projection without granting host execution authority.

## CLI Crate

`crates/lychnos-cli` is currently a minimal executable proving that an application can depend on `lychnos-core`.

It will later become a development and diagnostic interface for simulation and runtime control.

## Dependency Direction

The dependency direction is intentional:

```text
lychnos-cli
     |
     v
lychnos-core
```

Future platform integrations, UI components, voice systems, and adapters may depend on the core.

The core must not depend on them.

## Foundation Simulation

The current machine-independent test pipeline is:

```text
MockCollector / Mock Event
   |
   v
Normalized Event
   |
   v
InMemoryEventBus
   |
   v
MockAnalyzer
   |
   v
ActionProposal
   |
   v
RuntimeController
   |
   v
DefaultPermissionPolicy
   |
   v
MockExecutor
   |
   v
AuditSink
```

This allows Lychnos architecture to be tested without shell interception, systemd, journal monitoring, Omarchy hooks, hardware monitoring, AI-provider calls, microphone access, GPU acceleration, or privileged commands.

## Security Boundary

AI reasoning is not an execution authority.

The intended long-term rule is:

```text
AI / Analyzer
      |
      v
Structured Action Proposal
      |
      v
Validation / Permission / Runtime Safety
      |
      v
Restricted Executor
```

Never:

```text
AI output
   |
   v
arbitrary shell execution
```

No real executor exists yet.

## Memory Ownership

Lychnos memory is authoritative.

AI providers receive context from Lychnos but do not own Lychnos identity or persistent memory.

```text
             Lychnos Memory
                  |
        +---------+---------+
        |         |         |
        v         v         v
     OpenAI    Local LLM   Future AI
```

## Runtime Safety

Game Mode and Disabled mode are architectural safety states rather than UI-only switches.

Current core behavior blocks actions in both modes.

Runtime-driven collector polling is also suppressed in both Game Mode and Disabled before a collector is called.

For already-started simulated work, Game Mode requests cooperative pause and Disabled requests latched cancellation. Timeout assessment is currently deterministic simulation only and does not perform forced termination.

Resource-budget structures now exist in the core, but real performance coexistence behavior must still be measured later on actual workloads.

## Platform Integration Boundary

Omarchy is the first integration target.

It is not part of the core.

```text
               Lychnos Core
                    |
          +---------+---------+
          |                   |
          v                   v
   Omarchy Adapter       Other Adapter
```

No real Omarchy collector has been implemented yet.

## Portable Lychnos

The future portable device is another Lychnos body, not a separate AI product.

Desktop and portable Lychnos are intended to share identity, memory schema, synchronization protocol, and security model.

Transport, device identity, encryption, and conflict resolution remain open design questions.

## Architecture Decisions

Significant architectural choices are recorded as Architecture Decision Records under `docs/decisions/`.

Later ADRs may supersede earlier decisions explicitly.
