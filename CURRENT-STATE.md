# Project Lychnos Current State

## Current Phase

**Phase 1 — Machine-Independent Core Foundation**

Development is taking place on the Omarchy X1 Pro, but the implemented core remains intentionally independent of Omarchy and machine-specific behavior.

Current development branch:

```text
feat/core-bootstrap
```

## Current Milestone

The first end-to-end machine-independent Lychnos processing pipeline is working.

```text
Normalized Event
      |
      v
In-Memory Event Bus
      |
      v
Mock Analyzer
      |
      v
Structured Action Proposal
      |
      v
Live Runtime Safety
      |
      v
Permission Policy
      |
      v
Mock Execution Boundary
      |
      v
Security Audit Record
```

## Implemented

### Repository and Rust workspace

- Cargo workspace
- Rust 2024 edition
- pinned Rust toolchain
- `rustfmt`
- strict Clippy checks
- committed `Cargo.lock`
- `lychnos-core` library crate
- `lychnos-cli` binary crate
- GitHub Actions CI quality gate
- CI pinned to Ubuntu 24.04
- CI uses `actions/checkout@v7`

### Event model

Implemented:

- event IDs
- timestamps
- source
- event kind
- severity
- sensitivity
- correlation IDs
- structured payloads

### Collector boundary

Implemented:

- generic `Collector` interface
- normalized-event handoff into the core
- explicit no-event state through `Ok(None)`
- collector-specific error propagation
- deterministic FIFO `MockCollector`
- `FoundationRuntime::collect_once(...)`
- separate collector and runtime failures through `CollectorCycleError<E>`
- end-to-end collector -> runtime -> analyzer -> safety -> permission -> audit tests
- runtime-aware collector suppression before polling
- Game Mode does not poll collectors
- Disabled mode does not poll collectors
- queued collector events remain pending while background work is suppressed
- explicit re-enable to Normal resumes collection
- live action safety still applies to already-normalized events entering through the direct runtime boundary

The collector abstraction is machine-independent. Real terminal, journal, systemd, hardware, and Omarchy-specific collectors are still intentionally deferred.

### Event bus

Implemented:

- in-process publish/subscribe
- multiple subscribers
- disconnected-subscriber cleanup
- publication reporting

The current implementation uses the Rust standard library and is intentionally replaceable.

### Structured action model

Implemented:

- action IDs
- action kinds
- required capabilities
- impact classifications
- risk classifications
- structured parameters
- proposer identity
- reasons
- source-event linkage

### Permission model

The default policy evaluates against the live runtime controller.

```text
Normal + read-only       -> Allowed
Normal + state-changing  -> Requires user approval
Normal + external        -> Requires user approval
Normal + privileged      -> Requires user approval
Normal + destructive     -> Requires user approval
Game Mode                -> Denied
Disabled                 -> Denied
```

### Runtime safety

Implemented:

- Normal mode
- Game Mode
- Disabled mode
- thread-safe runtime controller
- immediate transition to Disabled
- Disabled fails closed
- explicit re-enable operation

### Audited runtime-mode transitions

Implemented:

- orchestrator-owned audited transitions into Game Mode
- orchestrator-owned audited transition to Disabled
- explicit audited re-enable to Normal
- `RuntimeModeChanged` security audit records
- structured `operation`, `from`, `to`, and `changed` audit details
- auditing of no-op and fail-closed transition requests
- end-to-end proof that disabling Lychnos blocks the next processing cycle and records both the mode transition and blocked permission decision

The `RuntimeController` remains the authority for state changes; the orchestrator adds the audit boundary around those transitions.

### Mock execution boundary

Implemented:

- safe mock executor
- no real system execution
- live permission evaluation
- `WouldExecute`
- `AwaitingUserApproval`
- `Blocked`

### Security audit model

Implemented:

- structured audit records
- event linkage
- action linkage
- structured details
- generic `AuditSink`
- append-only in-memory implementation
- permission/execution audit integration

Persistent tamper-resistant audit storage is not implemented yet.

### Lychnos-owned memory

Implemented:

- versioned memory schema
- memory IDs and kinds
- structured content
- provenance
- originating device ID
- memory scope
- sensitivity
- confidence
- sync revision metadata
- tombstone metadata
- generic `MemoryStore`
- in-memory implementation

Permanent storage is not selected yet.

### Configuration

Implemented:

- typed TOML configuration
- schema version validation
- unknown-field rejection
- runtime startup mode
- memory enable/disable
- audit enable/disable
- cloud-request privacy switch

Default cloud requests are disabled.

### Configuration loading boundary

Implemented:

- generic `ConfigSource` interface
- machine-independent `ConfigLoader`
- explicit distinction between missing configuration and source failure
- validated fallback to local-first defaults when no explicit configuration is supplied
- separate source errors and parse/validation errors through `ConfigLoadError<E>`
- deterministic in-memory configuration sources in tests

The core still deliberately does not choose a permanent config path, environment-variable precedence, CLI override precedence, secrets storage, or live reload behavior.

### ID and time providers

Implemented:

- generic `IdProvider`
- typed ID generation for events, actions, audit records, memory records, and devices
- deterministic `SequenceIdProvider` for tests and simulations
- generic `TimeProvider`
- deterministic `FixedTimeProvider` for tests
- operating-system-backed `SystemTimeProvider`
- typed event, audit, and memory timestamp helpers

The CLI now uses provider-generated event and audit IDs and real system wall-clock time instead of hard-coded values.

The sequence provider is intentionally not the final globally unique multi-device ID strategy.

### Foundation runtime orchestrator

Implemented:

- machine-independent `FoundationRuntime<I, T>`
- runtime-owned event bus and internal subscription
- runtime-owned safety controller
- runtime-owned in-memory audit log
- injected ID and time providers
- `observe(...)` for creating and processing a normalized event
- `process_event(...)` for already-normalized collector/adapter input
- `FoundationCycle` result containing the event, proposal, execution outcome, publish report, and audit count

The CLI now delegates the full foundation pipeline to `FoundationRuntime` instead of manually constructing the event bus, analyzer, runtime controller, executor, and audit flow itself.

### Foundation simulation

Implemented and tested:

```text
Mock event
-> bus
-> mock analyzer
-> action proposal
-> runtime controller
-> permission policy
-> mock executor
-> audit record
```

Tests prove that Game Mode and Disabled mode block the flow at the authorization boundary.

### CLI foundation simulation

The `lychnos-cli` executable now drives the same machine-independent foundation pipeline from the terminal.

Supported simulation modes:

```text
Normal    -> WOULD EXECUTE (mock only)
GameMode  -> BLOCKED (GameMode)
Disabled  -> BLOCKED (Disabled)
```

Each CLI simulation writes a security audit record.

Example commands:

```bash
cargo run -p lychnos-cli
cargo run -p lychnos-cli -- game
cargo run -p lychnos-cli -- disabled
```

This remains a safe simulation. The CLI does not execute real system actions.

## Automated Tests

Current expected test count:

```text
68
```

Current quality gate:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

All currently pass.

GitHub Actions runs the same quality gate automatically on pushes and pull requests into `main`, with manual workflow dispatch also available.

## Architecture Decision Records

Accepted ADRs currently cover:

```text
0001 Core module boundaries
0002 Normalized event envelope
0003 In-memory event bus
0004 Structured actions and permission boundary
0005 Security audit model
0006 Model-independent memory
0007 Configuration system
0008 Runtime safety controller
0009 Live runtime permission enforcement
0010 Mock execution boundary
0011 Audited mock execution
0012 Foundation simulation pipeline
0013 ID and time provider abstractions
0014 Foundation runtime orchestrator
0015 Audited runtime mode transitions
0016 Configuration loading boundary
0017 Collector boundary and foundation pipeline
0018 Runtime-aware collector suppression
```

## Intentionally Mocked

The following currently exist only as machine-independent test implementations:

- analyzer
- executor
- event bus transport
- audit persistence
- memory persistence
- platform-specific collectors

This is intentional.

## Not Implemented Yet

Lychnos does **not** currently have:

- real terminal monitoring
- real shell integration
- systemd monitoring
- journal monitoring
- Omarchy-specific collectors
- hardware telemetry
- microphone capture
- speech-to-text
- text-to-speech
- graphical orb UI
- real OpenAI/ChatGPT provider integration
- local LLM integration
- GPU/iGPU acceleration
- real privileged execution
- real shell-command execution
- persistent memory storage
- persistent audit storage
- portable-device synchronization
- automatic Game Mode detection
- gaming/streaming coexistence measurements
- anti-cheat coexistence testing

## Security State

No component currently executes real system actions.

There is no privileged Lychnos daemon.

There is no direct model-to-shell path.

Runtime safety is checked at the permission and mock-execution boundary.

A future real executor must re-check runtime state immediately before actual execution.

## Open Architectural Decisions

Still intentionally undecided:

- permanent event bus implementation
- async runtime strategy
- bounded queues and backpressure
- persistent memory backend
- persistent audit backend
- audit integrity mechanism
- AI provider interface details
- OpenAI authentication and integration mechanism
- local LLM runtime
- UI toolkit
- voice stack
- portable synchronization protocol
- production globally unique ID strategy
- device identity format
- at-rest encryption
- privileged helper design
- packaging
- installation
- automatic updating
- automatic Game Mode detection

## Next Foundation Milestones

Immediate next work should remain machine-independent.

Likely next steps:

1. resolve the security-audit versus diagnostic-logging configuration boundary
2. expand canonical project documentation
3. perform the final Phase 1 boundary and quality review before real Omarchy integration

Real Omarchy integration should begin only after these core boundaries are stable enough to connect safely.

## Hardware Context

The Omarchy X1 Pro is now available as the real Lychnos development machine.

That removes the earlier hardware-access blocker.

However, the architecture rule remains:

**platform-independent foundations come first; Omarchy-specific behavior is added through explicit adapters rather than leaking into the core.**
