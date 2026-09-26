# Project Lychnos Roadmap

This roadmap describes the intended development sequence for Project Lychnos.

It is directional rather than a promise of dates. Architectural decisions may refine individual milestones while preserving the core safety and portability principles.

## Phase 0 — Canon and Repository Foundation

Status: **Complete**

Goals:

- establish the public repository
- define the product baseline
- lock the canonical visual identity
- choose Rust for the machine-independent core
- establish architecture decision records
- establish CI and reproducible quality checks
- keep Omarchy as the first integration rather than the core itself

Key outputs include:

- Rust workspace
- pinned toolchain
- GitHub Actions quality gate
- canonical visual reference
- architecture documentation
- ADR process

## Phase 1 — Machine-Independent Core Foundation

Status: **Complete**

Goal:

Build the safety, data, and orchestration boundaries before connecting Lychnos to real operating-system observation or execution.

Implemented foundation areas include:

- normalized event model
- in-memory event bus
- collector boundary and deterministic mock collector
- runtime-aware collector suppression
- structured action proposals
- capability, impact, and risk metadata
- live runtime permission evaluation
- Normal, Game Mode, and Disabled runtime states
- audited runtime-mode transitions
- mock execution boundary
- dedicated mandatory security audit model
- separate configurable diagnostics model
- Lychnos-owned model-independent memory interface
- typed versioned configuration
- configuration loading boundary
- ID and time provider abstractions
- foundation runtime orchestrator
- safe CLI simulation
- automated CI quality gate

Phase 1 exit criteria:

- canonical project documentation is internally consistent
- full workspace quality gate passes
- no direct AI-to-shell execution path exists
- Game Mode and Disabled semantics are enforced at core boundaries
- security audit and diagnostics are clearly separated
- remaining deferred choices are explicitly documented rather than accidentally implied
- feature branch is reviewed through a pull request before merge to `main`

## Phase 2 — Local Simulator and Prototype Runtime

Status: **Current**

Goal:

Turn the foundation into a richer local development environment while keeping real host authority behind explicit boundaries.

Several interaction features originally listed in Phase 4 were deliberately pulled forward as isolated Phase 2 prototypes because they validate the provider, presentation, voice, and safety contracts without granting host execution authority. This includes the floating shell, approval UI, local STT/TTS, provider-neutral persona/interaction, capability-aware intelligence routing, the first Omarchy LocalBrain adapter, and bounded initiative.

Likely work:

- richer deterministic collectors and scenarios (scripted event/empty/error/recovery flow now implemented)
- multi-event simulations (stateful multi-step scenario runner now covers collection, runtime transitions, and pending-action lifecycle)
- analyzer/provider interface design
- explicit pending-action lifecycle with exact approval, rejection, and system-driven cancellation semantics, including deterministic scenario coverage
- configuration consumption by the application runtime (startup mode and diagnostics enablement now wired through typed config)
- diagnostics integration (runtime emission now wired independently from mandatory security audit)
- persistent local development state where justified
- failure injection and recovery tests (collector failure/recovery slice implemented)
- pending-action supersession now cancels the older request explicitly and audits it as system-driven cancellation
- cancellation semantics for work already in progress when Disabled is entered (machine-independent cancellation leases, acknowledgement/completion states, deterministic tracked-work scenarios, and simulation-only lifecycle audit/reporting now implemented; real executor cooperation remains pending)
- Game Mode semantics for already-running work (reversible pause request, explicit pause acknowledgement, explicit resume, and Disabled precedence now implemented in simulation)
- deterministic cooperation-window assessment for delayed acknowledgement, timeout observation, cancellation-unavailable behavior, and completion races without choosing a production timer/escalation mechanism
- tracked mock running-work lifecycle through the foundation runtime and deterministic scenario runner
- resource-budget instrumentation abstractions for later Game Mode testing (typed observations, explicit units, mode-specific budgets, mismatch handling, and in-memory observation log now implemented; real sampling remains deferred)
- read-only presentation-state projection for an early visual-shell prototype without granting UI execution authority (implemented)
- isolated Omarchy/Wayland desktop visual-shell prototype using GTK4 + layer shell, consuming presentation-domain state only (implemented with a temporary procedural body; canonical asset integration remains)
- lightweight floating/idle animation plus drag-to-move, remembered presentation position, and compact/expanded status-card ergonomics for the visual prototype (implemented); canonical body-asset integration remains

This phase should make it possible to exercise Lychnos behavior end to end before granting it meaningful access to the host system.

Near-term Phase 2 priority now favors two foundations that make Lychnos feel persistent and ambient rather than merely conversational:

- keep durable Lychnos-owned memory conservative and user-controlled: explicit capture/filtered recall now work; next add inspection/forget controls before considering inferred memory;
- the first real read-only ambient/context collector, feeding normalized context into the existing initiative and analysis boundaries.

The context-triggered initiative scheduler is intentionally conservative: meaningful context opens a consideration window, while user idleness only determines whether interruption is appropriate.

## Phase 3 — Real Omarchy and Hardware Integration

Status: **Planned**

Goal:

Connect stable core boundaries to the real development machine through explicit adapters.

Initial integration candidates:

- Omarchy shell and terminal events
- systemd state
- journal events
- selected hardware telemetry
- configuration discovery appropriate to the host platform

Security requirements:

- collectors normalize untrusted native input before it reaches core logic
- collectors remain suppressible by runtime state
- no persistent elevated privileges
- no unrestricted model-generated shell
- meaningful state-changing actions still require approval
- a future real executor re-checks live runtime state immediately before execution
- privileged operations, if ever required, use a narrow capability-limited helper rather than a privileged AI process

Performance work:

- measure idle overhead
- measure Game Mode overhead
- test gaming FPS and latency impact
- test streaming coexistence
- evaluate thermals and power usage
- evaluate anti-cheat-sensitive coexistence carefully

## Phase 4 — Interaction Layer and Intelligence Providers

Status: **Partially validated early; remaining work planned**

Goal:

Complete Lychnos' recognizable user-facing presence and provider ecosystem while keeping identity, memory, safety, and initiative Lychnos-owned.

Pulled forward and already validated during Phase 2:

- provider-neutral conversation interface;
- canonical provider-independent persona;
- floating desktop body and speech-bubble/chat surface;
- explicit approval UI;
- microphone input and Push-to-Talk capture;
- local Whisper speech-to-text;
- local Piper text-to-speech;
- spoken voice replies;
- capability-aware LocalBrain routing;
- first Omarchy/Linux llama.cpp LocalBrain adapter;
- bounded, context-triggered initiative architecture;
- Normal, Game Mode, and Disabled presentation/control behavior.

Remaining Phase 4 work includes:

- memory inspection/forget controls, richer relevance retrieval/consolidation, sensitive-memory policy, and provider-specific disclosure controls;
- supported AccountAgentBridge integrations where provider account/subscription mechanisms permit them;
- optional BYOK provider adapters without making paid APIs a baseline dependency;
- wake-word activation and longer voice-session behavior;
- user-facing intelligence, persona, voice, and output-mode settings;
- visible provider/fallback state and degraded-mode UX;
- additional provider/runtime adapters across supported bodies;
- richer speech interruption/ducking and output-device behavior.

The provider remains a reasoning component, never an execution authority.

## Phase 5 — Portable Lychnos and Shared Identity

Status: **Planned**

Goal:

Create a pocket-sized second body for the same Lychnos identity.

Expected portable capabilities:

- battery-powered operation
- microphone and speaker
- small face/display
- local speech-to-text and text-to-speech
- offline local model capability
- network/hotspot connectivity when required
- synchronization with desktop Lychnos

Major design work still required:

- portable identity protocol
- device identity
- authenticated pairing
- encrypted synchronization
- conflict resolution
- memory revision strategy
- offline/online reconciliation
- secure loss/recovery behavior

## Cross-Cutting Work

These areas span multiple phases and should be introduced when concrete requirements justify them:

- durable-memory backends across bodies, retrieval policy, and synchronization
- persistent audit backend
- tamper-evident audit integrity
- at-rest encryption
- globally unique multi-device IDs
- bounded queues and backpressure
- async runtime strategy
- packaging and installation
- secure updates
- release signing
- vulnerability reporting
- contributor documentation
- automated Game Mode detection

## Deliberately Unscheduled Decisions

The project does not need to choose these prematurely:

- final event-bus technology
- final database engine
- final local-model runtime
- final UI toolkit
- final voice stack
- final portable synchronization transport
- privileged-helper implementation
- production update mechanism

The rule is to preserve clean interfaces now and choose concrete technology when the requirements are real.

## Development Rule

Each phase should be built incrementally:

```text
small boundary
   |
tests
   |
quality gate
   |
document decision
   |
integrate next boundary
```

Real system authority should increase only after the layer beneath it is understandable, tested, and auditable.
