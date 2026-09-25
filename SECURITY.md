# Project Lychnos Security Model

## Purpose

Lychnos is intended to observe permitted system context, reason about it, and eventually propose or perform approved actions.

That makes security architecture a core product requirement.

This document describes the current security model and the invariants future integrations are expected to preserve.

It is not yet a vulnerability-disclosure policy.

## Security Principles

Lychnos follows these baseline rules:

- local-first by default
- least privilege
- explicit user authority for meaningful actions
- no direct AI-to-shell execution path
- live runtime safety checks
- immediate disable as a runtime invariant
- non-essential work suppressed in Game Mode
- mandatory security audit
- diagnostics kept separate from security audit
- no persistent elevated AI process
- platform-specific behavior isolated behind adapters
- untrusted external input normalized before entering core logic

## Trust Boundaries

The intended long-term trust flow is:

```text
External / Platform Input
          |
          v
      Collector
          |
          v
   Normalized Event
          |
          v
        Core
          |
          v
   Analyzer / AI
          |
          v
Structured Action Proposal
          |
          v
Validation + Runtime Safety + Permission
          |
          v
Restricted Executor
          |
          v
Security Audit
```

AI reasoning is deliberately inside the decision process but outside the final authority boundary.

## AI Is Not an Execution Authority

A model may explain, classify, summarize, or propose an action.

A model must not directly turn arbitrary generated text into shell execution.

The intended path is:

```text
model output
   |
structured proposal
   |
validated capability + parameters
   |
live runtime state
   |
permission decision
   |
restricted executor
```

Never:

```text
model text -> shell
```

The current foundation contains no real executor.

## Runtime Safety

Lychnos currently defines three runtime modes:

- `Normal`
- `GameMode`
- `Disabled`

### Normal

Ordinary foundation processing is allowed.

Read-only actions may pass the default policy.

State-changing, external, privileged, and destructive actions require user approval.

### Game Mode

Actions are denied.

Non-essential collector polling is suppressed before collectors run.

The long-term purpose is to minimize interference with gaming, streaming, thermals, latency, and anti-cheat-sensitive workloads.

### Disabled

Actions are denied.

Ordinary collector polling is suppressed.

Disabled fails closed against ordinary Game Mode transitions.

Returning to Normal requires an explicit enable operation.

A future real executor must also handle cancellation or containment of work that had already started before Disabled was entered.

## Permission Boundary

Permission decisions consult the live runtime controller rather than trusting a stale runtime snapshot.

This reduces the chance that an action analyzed under one mode executes after the runtime has moved into a stricter mode.

A future real executor must re-check live runtime state immediately before actual execution.

## Collector Security

Collectors are adapters between native platform data and Lychnos events.

Future collectors may read sources such as:

- terminal activity
- journals
- systemd
- hardware telemetry
- Omarchy-specific hooks

Collector rules:

- treat external/native data as untrusted
- normalize data before handing it to the core
- avoid side effects while observing
- remain unprivileged where practical
- obey runtime suppression
- do not smuggle platform-specific execution authority into the core

The current collector implementation is a deterministic in-memory mock only.

## Security Audit

Security audit records are a mandatory core mechanism.

Ordinary configuration cannot disable the security audit trail.

The audit domain is intended to record security-relevant behavior such as:

- permission decisions
- runtime-mode transitions
- future execution attempts
- future execution results
- other security-significant activity

The current audit sink is append-only through its public API and exists only in memory.

Persistent storage, cryptographic integrity, retention, export, and tamper evidence are future work.

## Diagnostics Are Different

Diagnostic logging exists for development and troubleshooting.

Diagnostics:

- use a separate domain model
- use a separate sink
- may be disabled
- may eventually use different retention and privacy rules

Diagnostic records must never be treated as a substitute for mandatory security audit records.

## Privilege Model

The Lychnos core should run without elevated privilege.

There is no privileged Lychnos daemon in the current design.

If future functionality genuinely requires privilege, the preferred design is a small, separately reviewable helper exposing narrow capabilities.

That helper should:

- perform only explicitly defined operations
- validate all parameters
- expose no arbitrary shell interface
- avoid accepting raw model output
- have the minimum required privilege
- be independently auditable

The AI process itself should not run as root.

## Memory and Privacy

Lychnos-owned memory is authoritative and independent from AI providers.

Sensitive context should remain local unless a user-enabled feature explicitly requires sending selected context to an external provider.

Cloud requests are disabled by default in the current configuration.

Enabling cloud requests does not automatically authorize all local data to leave the device.

Context selection remains a separate responsibility.

Future persistent memory requires decisions on:

- at-rest encryption
- device identity
- backup
- export
- deletion/tombstone behavior
- portable synchronization security

## Provider Boundary

AI providers are replaceable.

No provider should own Lychnos identity, persistent memory, permissions, or execution authority.

Provider compromise or failure should therefore not automatically imply unrestricted host control.

Future provider integrations should minimize transmitted context and make provider use visible and configurable.

## Portable Device Security

The future portable Lychnos introduces additional trust boundaries.

Before portable synchronization is enabled, the project needs explicit designs for:

- authenticated pairing
- device identity
- encrypted transport
- replay protection
- conflict resolution
- lost-device handling
- local secret storage
- revocation

Portable synchronization must not be bolted onto the memory system as an unauthenticated convenience channel.

## Configuration Security

Configuration is typed and versioned.

Unknown fields are rejected.

Source failures are distinguished from missing configuration.

A failed configuration source must not silently become a permissive default.

Security auditing is not configurable off.

Secrets are not intended to live in ordinary TOML configuration.

A dedicated secret-storage strategy remains future work.

## Current Security Limitations

The current foundation deliberately does not provide:

- real shell execution
- real privileged execution
- persistent audit storage
- audit tamper resistance
- persistent memory storage
- at-rest encryption
- real OS collectors
- AI-provider integration
- microphone capture
- portable synchronization

These are deferred boundaries, not silently assumed protections.

## Future Security Work

Before Lychnos gains meaningful host authority, the project will need to address:

- real-executor cooperation, acknowledgement, timeout, and escalation for already-running work on Game Mode/Disabled transitions
- bounded queues and resource exhaustion
- hostile collector input
- persistent audit integrity
- secure credential storage
- real executor capability design
- privileged-helper threat model, if required
- package and update authenticity
- dependency and supply-chain review
- portable-device pairing and revocation
- privacy-preserving diagnostics
- security regression tests

## Security Reporting

A formal private vulnerability-reporting channel has not yet been established.

Until one exists, do not place credentials, private keys, personal data, or undisclosed exploit details into public repository issues.

A private reporting process should be established before Lychnos ships releases with meaningful system authority.
