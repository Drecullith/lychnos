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

### audit

Defines structured security audit records and the `AuditSink` interface.

The security audit trail is mandatory and cannot be disabled through ordinary configuration.

The current `InMemoryAuditLog` is append-only through its public API.

Persistent, tamper-resistant audit storage is future work.

### diagnostics

Defines ordinary troubleshooting and development records separately from the security audit system.

The current `InMemoryDiagnosticLog` is a foundation implementation behind the `DiagnosticSink` interface.

Diagnostics may be disabled independently. They do not replace mandatory security audit records.

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

Real performance coexistence behavior must be measured later on actual workloads.

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
