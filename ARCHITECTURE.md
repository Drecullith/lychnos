# Project Lychnos Architecture

## Status

This document describes the current architectural baseline of Project Lychnos.

Lychnos remains foundation-first, but Phase 2 now includes deliberately isolated live prototypes for the Omarchy shell, local voice, and a capability-aware LocalBrain adapter. Real ambient host collectors and privileged execution remain intentionally deferred behind the platform and security boundaries.

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

Pending-approval controls use a separate versioned `CompanionControlEnvelope`. The shell emits only an action ID, an opaque proposal binding, and an Approve/Reject intent into a session-local inbox. The runtime owner retrieves the real pending `ActionProposal`, verifies the binding against that exact proposal, and only then invokes the existing approval or rejection boundary. The shell still receives no approval grant, executor, runtime controller, or mutation handle.

The current proposal binding is a SHA-256 digest over the in-process proposal representation. It is a stale-decision/freshness guard for the Phase 2 session transport, not an authentication mechanism or persistent cross-version identifier.

This allows an early visual shell to follow live core state and collect explicit user decisions without becoming an execution authority.

### interaction

Defines provider-neutral conversation requests, responses, input provenance, and the replaceable `ConversationProvider` boundary.

Typed input, push-to-talk, wake-word, and voice-session sources share one normalized interaction model. The current Omarchy runtime now uses the real local LocalBrain adapter when available and retains `MockConversationProvider` only as a deterministic fallback.

### persona

Defines Lychnos-owned conversational identity independently from any model provider.

The canonical `lychnos.default.v1` profile supplies the companion name, role, traits, conversation style, initiative style, and operating principles to future provider adapters. Providers may consume Lychnos persona/context, but they do not own Lychnos identity.

### initiative

Defines a bounded provider-neutral path for proactive Lychnos speech. An `InitiativeProvider` may propose an inspectable message candidate, but the core-owned `InitiativePolicy` decides whether it may surface based on runtime mode, user interaction, initiative mode, priority, and cooldown.

Initiative carries text only. It has no executor, approval grant, shell-command path, or mutation authority. Game Mode and Disabled mode suppress AI initiative entirely.

The current runtime now drives this boundary with a conservative, context-triggered scheduler. Initiative requires real session context, an explicit unconsidered InitiativeTrigger, at least 60 seconds of user quiet, Normal runtime mode, and no pending approval. User activity only delays interruption; silence alone never creates a thought opportunity. Successful conversation turns currently create ConversationFollowUp triggers, while future collectors and durable-memory retrieval can supply diagnostic, context, or memory triggers. Unchanged context is considered at most once. Surfaced thoughts use the normal response/TTS path and are added to session context only after they were actually shown.

### intelligence

Defines capability-aware routing without coupling Lychnos to one model, runtime, operating system, or commercial provider.

The core models broad body platforms, normalized hardware capabilities, local-brain resource classes, portable model manifests, and Local / AccountBridge / optional ApiByok routes. Local is the no-subscription baseline. Platform adapters decide how to probe real hardware and which runtime/model implementation fits the current body.

A desktop may therefore run a larger GGUF model through llama.cpp while an Android, iOS, or Pocket body selects a smaller or platform-native model, all behind the same Lychnos conversation/persona/initiative contracts.

The first Omarchy adapter now runs a loopback-only llama.cpp child server behind the existing `ConversationProvider` and `InitiativeProvider` boundaries. Model selection is machine-local, local HTTP is authenticated with an ephemeral runtime token, hidden `<think>` content is stripped before presentation, and failure to start the local brain falls back to the deterministic mock rather than making Lychnos unusable.

### visual shell prototype

`prototypes/lychnos-shell` is an isolated Omarchy/Wayland experiment built with GTK4 and gtk4-layer-shell.

It is deliberately outside the main Cargo workspace so Linux desktop native dependencies do not become requirements for the platform-independent core or its standard CI.

The shell is normally non-keyboard-interactive, reserves no screen space, and renders the validated canonical Lychnos body asset with state-driven cyan expressions plus a compact status card. Opening the chat panel temporarily uses layer-shell `OnDemand` keyboard mode so the user can type, then returns to `None` when chat closes.

It follows the versioned read-only presentation snapshot produced by the separately owned runtime and sends authority-free approval and interaction requests through versioned session-local transports. The current JSON/session-file transport is intentionally replaceable and is not a final IPC decision.

This prototype is not a final UI-toolkit decision.

### voice

Defines platform-neutral audio-input identities, explicit capture-control messages, runtime-confirmed capture status, and voice-activation modes without depending on PipeWire, ALSA, or another host audio API.

The Omarchy runtime discovers `Audio/Source` nodes through structured `pw-dump` data and marks the active default source using `wpctl`. Push-to-talk uses explicit start/stop requests, a bounded 60-second capture window, runtime acknowledgements, and temporary 16 kHz mono WAV files under the session runtime directory. Machine-local source-port and input-level preferences are applied immediately before capture.

### speech

Defines replaceable speech input/output boundaries: `SpeechToTextProvider` for recognition and `SpeechSynthesizer` for spoken output, plus normalized transcript, synthesis-request, and Lychnos-owned voice-profile types.

The current Omarchy STT adapter runs multilingual whisper.cpp locally. Finalized PTT audio is transcribed locally, converted into a normal `InteractionSource::PushToTalk` conversation request, then deleted. Whisper non-speech tokens are suppressed, and pure annotation outputs such as `(upbeat music)` or `[Music]` are treated as no recognized speech rather than literal user text.

The current Omarchy TTS adapter runs Piper locally. The canonical `lychnos.voice.default.v1` identity is currently bound to a British male medium Piper model. Voice-originated replies are published as text and also queued to a single background speech worker for synthesis and PipeWire playback; synthesized WAV files are deleted afterward.

Wake-word and voice-session capture will reuse the same normalized input, STT, persona, conversation, and speech-output path.

### memory

Defines Lychnos-owned, model-independent memory records and the `MemoryStore` interface.

The current implementation is an in-memory store.

Separately, the first LocalBrain adapter keeps a bounded six-turn session conversation window so follow-ups and initiative have natural short-term context. That transient window is cleared on runtime restart and is deliberately **not** treated as durable Lychnos memory.

The authoritative persistent memory model belongs to Lychnos rather than any AI provider.

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

## Runtime and CLI Crates

`crates/lychnos-runtime` is the current unprivileged desktop runtime owner. It owns `FoundationRuntime`, the canonical persona, presentation publication, approval/rejection consumption, and conversation-provider calls.

`crates/lychnos-cli` remains a development and diagnostic executable for simulation and explicit test scenarios.

The Omarchy shell is a separate presentation process and does not own runtime authority.

## Dependency Direction

The dependency direction is intentional:

```text
lychnos-runtime ----+
                    |
lychnos-cli --------+--> lychnos-core
                    |
lychnos-shell ------+
```

Platform integrations, UI components, voice systems, and adapters may depend on the core.

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
