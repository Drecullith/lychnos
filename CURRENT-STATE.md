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

The floating Lychnos desktop shell now follows **live read-only state from a separately owned `FoundationRuntime`**.

The runtime projects `CompanionPresentationState`, wraps it in a versioned `CompanionPresentationEnvelope`, and publishes an atomic session-local snapshot. The GTK shell consumes only that read-only projection and changes expression/status without gaining execution authority.

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
- `lychnos-runtime` user-level runtime binary crate
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
- machine-independent work-start leases for already-started work
- new work leases are issued only in Normal mode
- each successful transition into Disabled advances a disable generation
- work that started before a Disabled transition observes cancellation requested
- explicit re-enable to Normal does not revive leases invalidated by an earlier Disabled transition
- work started after re-enable receives a fresh uncancelled lease

Game Mode continues to block new actions and non-essential background work, but cancellation/pause semantics for work that was already running when Game Mode begins remain intentionally undecided.

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
- simulation-only `MockRunningWork` representation for already-started work
- observable running-work states `Running`, `PauseRequested`, `PausedForGameMode`, `CancellationRequested`, `CancellationUnavailable`, `Completed`, and `StoppedAfterCancellation`
- explicit executor-owned acknowledgement for Game Mode pause and Disabled cancellation
- explicit resume after Game Mode; paused work never silently resumes
- completion-after-cancellation-request race represented separately from confirmed cancellation
- stable machine-readable runtime/work-state labels
- structured `MockWorkLifecycleSnapshot` reporting current mode, state, terminal status, and cooperation-pending status
- dedicated `SimulationWorkLifecycleChanged` audit records for successful mock-work transitions
- simulation lifecycle audit details explicitly identify `simulation = true` and do not use real-execution audit kinds
- invalid lifecycle transitions do not falsely append lifecycle-change audit records
- Disabled-mode cancellation is modeled without shell commands, process control, file writes, or other host effects

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

Pending actions are still in-memory only. Expiry, persistence, and restart recovery are not implemented yet.

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

Rejection is intentionally distinct from system-driven cancellation.

### System-driven pending action cancellation

Implemented:

- dedicated `ActionCancelled` security-audit event
- `PendingActionCancellationReason` with stable audit representations
- exact-proposal `cancel_pending_action(...)` boundary
- cancellation uses the runtime-owned `foundation-runtime` actor rather than a user actor label
- unknown proposals cannot be cancelled
- stale proposals cannot cancel newer replacements with the same action ID
- structurally different same-ID proposals cancel the older pending proposal as `Superseded` before the replacement is evaluated
- cancellation audit records preserve action ID, source event when present, action kind, capability, impact, risk, and cancellation reason
- cancellation remains available while Disabled
- explicit user rejection remains semantically and audibly separate from system cancellation
- entering Disabled does not automatically cancel all pending proposals; blocked approval attempts still retain pending state for later retry

Pending-action cancellation remains separate from cancellation of already-started work.

Already-started work now has a separate machine-independent runtime-lease boundary. Game Mode produces a reversible pause request; Disabled permanently invalidates older leases and produces an irreversible cancellation request. The simulation distinguishes requested, acknowledged, unavailable, completed, paused, resumed, and stopped-after-cancellation states. `FoundationRuntime` retains mock work by action ID, exposes structured lifecycle snapshots, and audits successful mock lifecycle transitions explicitly as simulation-only. No operating-system task is started or stopped by this prototype.

### Deterministic running-work cooperation windows

Implemented:

- explicit cooperation request classes for Game Mode pause and Disabled cancellation
- pure `MockWorkCooperationAssessment`
- caller-supplied elapsed milliseconds and timeout budget
- `Waiting`, `TimedOut`, `Satisfied`, `Unavailable`, `SupersededByDisabled`, `CompletedBeforeCooperation`, and `NotRequested` outcomes
- timeout observation does not force a lifecycle state change
- late acknowledgement after an observed timeout remains representable
- deterministic scenario coverage for delayed pause acknowledgement
- deterministic scenario coverage for late Disabled-cancellation acknowledgement
- deterministic cancellation-unavailable and later-completion story

This boundary deliberately does not choose wall-clock scheduling, an async runtime, production timeout values, process termination, or escalation behavior. Real executor cooperation, persistence, rollback, and escalation remain future work.

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

### Resource-budget instrumentation

Implemented:

- machine-independent `resource` module
- stable resource metric names
- explicit units: bytes, microseconds, counts, and basis points
- typed `ResourceObservation` values tagged with runtime mode and component
- mode-specific `ResourceBudget`
- explicit metric/unit/runtime-mode mismatch errors
- exact within-budget headroom and exceeded-budget excess
- generic `ResourceSink`
- deterministic `InMemoryResourceLog`

No real resource sampler is implemented yet. The core deliberately does not choose Linux telemetry sources, production metric names, measurement windows, thresholds, sampling cadence, background tasks, or an async runtime. Phase 3 adapters can later supply real Omarchy measurements through this boundary.

### Read-only companion presentation state

Implemented:

- machine-independent `presentation` module
- `CompanionPresentationState` as an owned read-only UI projection
- projected runtime mode and diagnostics-enabled state
- projected pending approvals containing descriptive action data only
- projected tracked mock-work state with terminal/cooperation-pending flags
- latest ordinary diagnostic projection when available
- `FoundationRuntime::presentation_state()`
- versioned `CompanionPresentationEnvelope` wire contract
- schema-version validation with unknown versions rejected
- JSON round-trip coverage for the authority-free projection
- tests proving projection does not mutate pending actions, mock work, runtime state, or audit history
- no approval grants, runtime controller, executor, mutable runtime reference, or direct action method exposed through the presentation model

The current Phase 2 transport publishes this read-only snapshot to session-local runtime storage. The transport is replaceable; the authority boundary is canonical.

### Omarchy floating visual-shell prototype

Implemented:

- isolated `prototypes/lychnos-shell` Rust binary outside the main Cargo workspace
- GTK4 + gtk4-layer-shell on the Omarchy/Hyprland development machine
- non-exclusive Wayland overlay surface with no reserved screen space
- validated canonical black/blue Lychnos body asset
- state-driven cyan expression overlay
- compact live status card
- live read-only snapshot consumption from a separately owned `FoundationRuntime`
- Normal, active-work, approval, Game Mode, Disabled, and warning/error visual mappings
- drag-to-move and remembered position
- double-click status toggle
- right-click see-through, status, position lock/reset, minimize, and close controls
- restart-safe shell preferences for status visibility, Ghost Mode opacity, position lock, and minimized/visible presence
- true click-through Ghost Mode using an empty GDK input region
- Omarchy top-bar recovery integration for both minimized and Ghost Mode states under `integrations/omarchy/bar/`
- user-level Omarchy installer that builds and installs the shell without sudo
- installed `lychnos` launcher command with start, stop, restart, status, and logs
- desktop application entry and repo-independent installed body/bar assets
- launcher recovery of the active Wayland session environment for remote/non-graphical parent shells
- pending-approval card with action reason, kind, risk, impact, Approve, and Reject controls
- versioned presentation-to-runtime control envelope written into a session-local inbox
- opaque SHA-256 proposal binding so stale same-ID UI decisions fail closed before runtime approval/rejection
- runtime-side consumer that re-checks the exact live pending proposal before using existing audited approve/reject APIs
- compact typed-chat panel with temporary on-demand keyboard focus
- versioned typed interaction request/response transport under the session runtime directory
- installed `lychnos-runtime` process that owns `FoundationRuntime`, persona, approvals, presentation publication, and conversation-provider calls
- installed launcher starts/stops/restarts both runtime and shell and reports both statuses
- deterministic local mock conversation provider for safe end-to-end interaction testing
- canonical provider-neutral `lychnos.default.v1` persona profile
- normalized interaction sources for Typed, Push-to-Talk, Wake Word, and Voice Session
- platform-neutral `AudioInputDevice` and voice-activation vocabulary
- Omarchy PipeWire microphone discovery through structured `pw-dump` data
- default-source detection through `wpctl` without opening or recording the microphone
- live runtime detection of the Ryzen/ALC245 default capture source on the X1 Pro
- lightweight idle float/pulse animation
- no executor, approval grant, runtime controller, or host-action authority in the shell

A foundation CLI demonstration now proves the end-to-end path:

```text
FoundationRuntime
      |
      v
CompanionPresentationState
      |
      v
CompanionPresentationEnvelope v1
      |
      v
session-local atomic snapshot
      |
      v
GTK shell
```

The current local JSON snapshot/polling transport is a Phase 2 prototype, not a final IPC or UI-toolkit decision.

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

`runtime.startup_mode` and `diagnostics.enabled` are currently consumed by the runtime. Memory, privacy, persistence, and provider-related settings will be wired into their owning subsystems incrementally.

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

### Deterministic multi-step scenario runner

Implemented:

- machine-independent `scenario` module
- ordered `ScenarioStep` operations
- collection steps against an existing collector
- runtime transition steps for Game Mode, Disabled, and explicit return to Normal
- one continuous `FoundationRuntime` across the entire scenario
- step-level collection failures that do not abort later scenario steps
- boxed foundation-cycle results to keep the result enum compact
- preservation of runtime mode, diagnostics, audit history, and collector queue position across steps
- deterministic proof that Game Mode pauses collection without consuming queued events
- deterministic proof that Disabled persists until explicit enable
- deterministic proof that a scenario can continue and recover after an injected collector failure
- accumulated runtime-transition audit history across a scenario
- proposal-evaluation steps for structured actions
- exact pending-action approval steps with actor labels
- exact explicit rejection steps with actor labels
- exact system-cancellation steps with structured cancellation reasons
- typed lifecycle results preserving `PendingActionError`
- deterministic approval story from pending state to mock advancement
- deterministic explicit-rejection story with `ActionRejected`
- deterministic system-cancellation story with `ActionCancelled`
- deterministic Disabled-mode approval retry: blocked approval retains pending state, explicit re-enable permits retry of the same exact proposal
- tracked mock running-work start, inspection, pause acknowledgement, resume, cancellation acknowledgement, cancellation-unavailable, and completion steps
- deterministic cooperation-window assessment steps with caller-supplied elapsed time and timeout budget
- deterministic delayed Game Mode cooperation and timeout story
- deterministic late Disabled-cancellation acknowledgement after timeout
- deterministic cancellation-unavailable and later normal-completion story

The scenario layer is simulation orchestration only. It delegates lifecycle authority to `FoundationRuntime` and does not bypass runtime safety, permission checks, approval boundaries, cancellation semantics, or security auditing.

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

Current expected test counts:

```text
Core workspace:       177
Runtime adapter tests:   7
GTK shell prototype:     5
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
0026 Deterministic multi-step scenarios
0027 System-driven pending action cancellation
0028 Pending action lifecycle scenarios
0029 Disabled-mode cancellation leases for running work
0030 Running-work lifecycle acknowledgement
0031 Game Mode running-work pause semantics
0032 Tracked mock running-work scenarios
0033 Simulation work lifecycle reporting and audit
0034 Deterministic running-work cooperation windows
0035 Resource budget instrumentation abstractions
0036 Read-only companion presentation state
0037 Isolated Wayland visual shell prototype
0038 Versioned read-only presentation snapshot transport
0039 Presentation control intents
0040 Provider-neutral interaction and persona
0041 User runtime and shell process split
0042 Normalized voice input and PipeWire discovery
0043 Local push-to-talk speech recognition
0044 Provider-neutral local text-to-speech
0045 Bounded character initiative
0046 Capability-aware intelligence routing across bodies
0047 First LocalBrain adapter on Omarchy
```

## Intentionally Mocked

The following currently exist only as machine-independent test implementations:

- analyzer
- executor
- event bus transport
- audit persistence
- memory persistence
- platform-specific collectors
- deterministic mock conversation provider remains only as the fallback when a real local brain cannot start

This is intentional.

## Not Implemented Yet

Lychnos does **not** currently have:

- real terminal monitoring
- real shell integration
- systemd monitoring
- journal monitoring
- Omarchy-specific collectors
- hardware telemetry
- production graphical orb/body UI (an isolated Omarchy visual prototype now exists)
- real OpenAI/ChatGPT provider integration
- live initiative provider/runtime scheduling (the bounded core policy now exists)
- additional LocalBrain adapters for Windows, macOS, Android, iOS, and Pocket bodies; Omarchy/Linux llama.cpp is now the first working adapter
- portable model-manifest/package mappings for non-llama runtimes
- real privileged execution
- real shell-command execution
- persistent memory storage
- persistent audit storage
- portable-device synchronization
- automatic Game Mode detection
- real executor cooperation with pause/cancellation requests
- running-work timeout and escalation policy
- real execution lifecycle auditing and persistence
- gaming/streaming coexistence measurements
- anti-cheat coexistence testing

## Security State

No component currently executes real system actions.

There is no privileged Lychnos daemon.

There is no direct model-to-shell path.

Runtime safety is checked at the permission and mock-execution boundary.

A future real executor must re-check runtime state immediately before actual execution.

Game Mode now exposes a reversible pause request to already-started simulated work, while Disabled exposes a latched cancellation request to work that began under an older runtime generation. The mock lifecycle can acknowledge those requests, but this acknowledgement is simulation state only: it does not mean an operating-system task actually paused or stopped, and it does not imply prior side effects were rolled back.

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
- final cross-platform local-model packaging strategy beyond the first working llama.cpp adapter
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

Immediate next work remains simulation-first around the now-live presentation boundary.

Likely next steps:

1. define a portable model-manifest/catalog flow for Tiny, Compact, Standard, and Large bodies and map additional OS/mobile adapters onto it
2. wire the bounded initiative policy into the runtime with real context and a conservative scheduler
3. add AccountAgentBridge adapters only where providers expose supported subscription/account mechanisms
4. add wake-word activation on the existing local voice pipeline
5. add explicit intelligence/voice/persona settings and user-tunable local/account routing
6. keep real collectors and host execution behind their adapter/security boundaries

## Hardware Context

The Omarchy X1 Pro is now available as the real Lychnos development machine.

That removes the earlier hardware-access blocker.

However, the architecture rule remains:

**platform-independent foundations come first; Omarchy-specific behavior is added through explicit adapters rather than leaking into the core.**
