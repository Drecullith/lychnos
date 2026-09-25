# Project Lychnos Current State

## Current Phase

**Phase 2 — Local Simulator and Prototype Runtime**

Phase 1 was merged into `main` through pull request #1 after the full local and GitHub Actions quality gates passed.

Development is taking place on the Omarchy X1 Pro, while Phase 2 continues to keep the core machine-independent until the simulator and safety boundaries are strong enough for real platform adapters.

Current development branch:

```text
feat/phase2-approval-flow
```

## Current Milestone

The first Phase 2 approval boundary is working: explicit approval can now be bound to one exact structured action proposal without weakening live runtime safety.

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
- dual licensing under MIT OR Apache-2.0
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
- deterministic `ScriptedCollector` for ordered event, empty-poll, and injected-error scenarios
- typed `InjectedCollectorError` for reproducible failure injection
- `FoundationRuntime::collect_once(...)`
- separate collector and runtime failures through `CollectorCycleError<E>`
- end-to-end collector -> runtime -> analyzer -> safety -> permission -> audit tests
- runtime-aware collector suppression before polling
- Game Mode does not poll collectors
- Disabled mode does not poll collectors
- queued collector events remain pending while background work is suppressed
- explicit re-enable to Normal resumes collection
- live action safety still applies to already-normalized events entering through the direct runtime boundary
- collector errors emit ordinary diagnostics while preserving the original typed collector error
- collector failures do not fabricate security-audit records
- deterministic tests prove recovery after an injected collector failure and continued event processing

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

### Bound action approval flow

Implemented:

- `ApprovalGrant` bound to the complete structured `ActionProposal`
- crate-private approval-grant construction
- approval grants are issued internally only after exact pending-proposal verification
- explicit `ActionApproved` security-audit records
- approving actor label captured in the approval audit record
- action kind, capability, impact, and risk captured at approval time
- approval grants consumed by the execution-evaluation call
- exact proposal matching rather than action-ID-only matching
- modified parameters or metadata invalidate an existing approval
- live runtime state is checked before approval can authorize the mock executor
- Game Mode and Disabled override an existing approval
- approval state recorded during permission evaluation

The current `approved_by` field is only a caller-supplied actor label. It is not yet authenticated user identity, and there is no approval expiry, cryptographic signature, persistent grant store, or cross-process replay protection.

No real system execution is introduced by the approval flow.

### Pending action approval runtime flow

Implemented:

- runtime-owned in-memory pending-action store
- `evaluate_proposal(...)` retains proposals that require approval
- exact pending proposal lookup by `ActionId`
- `approve_pending_action(...)` rejects unknown proposals
- stale or modified proposals are rejected before approval issuance
- approval grant issuance is internal to the runtime
- approved proposals are re-checked against live runtime safety
- successful mock advancement removes the proposal from pending state
- runtime-blocked approved proposals remain pending
- pending approval flow is covered by end-to-end runtime tests

Pending actions are still in-memory only. Expiry, persistence, restart recovery, and system-driven cancellation are not implemented yet.

### Explicit pending action rejection

Implemented:

- `reject_pending_action(...)` for explicit negative consent
- exact pending-proposal verification before rejection
- unknown proposals cannot be rejected
- stale proposals cannot reject newer replacements that reuse an action ID
- successful rejection removes the pending proposal
- dedicated `ActionRejected` security-audit records
- rejecting actor label captured in the audit record
- action kind, capability, impact, risk, action ID, and source-event context preserved
- rejection remains available in Normal, Game Mode, and Disabled
- pending removal occurs before audit append so future audit-storage failure cannot undo user refusal

Rejection is intentionally distinct from future cancellation semantics.

### Security audit model

Implemented:

- structured audit records
- event linkage
- action linkage
- structured details
- generic `AuditSink`
- append-only in-memory implementation
- permission/execution audit integration
- security auditing is mandatory and cannot be disabled through ordinary configuration

Persistent tamper-resistant audit storage is not implemented yet.

### Diagnostic logging boundary

Implemented:

- separate `DiagnosticLevel`
- separate `DiagnosticRecord`
- generic `DiagnosticSink`
- in-memory diagnostic log
- diagnostics configuration independent from the security audit trail
- runtime-owned diagnostic log
- `diagnostics.enabled` consumed by `FoundationRuntime::from_config(...)`
- startup diagnostics when enabled
- action-evaluation diagnostics for mock-allowed, awaiting-approval, and runtime-blocked outcomes
- read-only diagnostic-log access from the foundation runtime
- explicit test proving diagnostics can be disabled while the mandatory security audit continues recording permission decisions

Diagnostics are for troubleshooting and development. They do not carry the security guarantees of the audit system and may be disabled independently.

Security auditing remains mandatory and independent of diagnostic configuration.

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
- schema version 2
- schema version validation
- unknown-field rejection
- runtime startup mode
- memory enable/disable
- diagnostic logging enable/disable
- mandatory security audit with no configuration disable switch
- cloud-request privacy switch
- explicit rejection of legacy schema version 1

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

### Typed configuration runtime consumption

Implemented:

- `FoundationRuntime::from_config(...)`
- live runtime startup mode derived from `LychnosConfig.runtime.startup_mode`
- typed startup coverage for Normal, Game Mode, and Disabled
- CLI now constructs typed `LychnosConfig` before creating the runtime
- CLI simulation arguments temporarily modify typed config rather than bypassing it
- filesystem/config-discovery policy remains outside the machine-independent core

Only `runtime.startup_mode` is consumed by the runtime so far. Memory, diagnostics, privacy, persistence, and provider-related settings will be wired into their owning subsystems incrementally.

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

### Canonical project documentation

Implemented and synchronized:

- `README.md`
- `VISION.md`
- `ARCHITECTURE.md`
- `ROADMAP.md`
- `SECURITY.md`
- `CURRENT-STATE.md`
- architecture decision records under `docs/decisions/`

`CONTRIBUTING.md` remains intentionally deferred until the repository is ready for a wider contributor workflow.

## Automated Tests

Current expected test count:

```text
97
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
0019 Mandatory security audit and separate diagnostics
0020 Bound action approval grants
0021 Pending action approval runtime flow
0022 Explicit pending action rejection
0023 Typed configuration runtime consumption
0024 Runtime diagnostics integration
0025 Deterministic collector failure scenarios
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

## Next Phase 2 Milestones

Immediate next work should remain machine-independent and simulation-first.

Likely next steps:

1. define system-driven cancellation semantics separately from explicit user rejection
2. expand deterministic simulation into richer multi-event scenarios
3. define cancellation semantics for already-running work before any real executor exists

Real Omarchy integration remains Phase 3 work and should begin only after these prototype runtime boundaries are stable enough to connect safely.

## Hardware Context

The Omarchy X1 Pro is now available as the real Lychnos development machine.

That removes the earlier hardware-access blocker.

However, the architecture rule remains:

**platform-independent foundations come first; Omarchy-specific behavior is added through explicit adapters rather than leaking into the core.**
