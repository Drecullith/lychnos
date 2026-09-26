# Project Lychnos — Canon

This file records stable product truths that future implementation work must preserve unless the project deliberately reopens them. Detailed implementation choices belong in ADRs.

## Identity

Lychnos is an open-source, local-first ambient AI companion platform.

Lychnos is not one model, one operating system, one device, or one commercial provider.

The same Lychnos identity is intended to move across desktop, phone, and future Pocket/portable bodies.

Lychnos owns identity, persona/character, memory, permissions and safety policy, initiative policy, normalized interaction semantics, and security/audit concepts.

Models and inference runtimes are replaceable intelligence components.

## Bodies and Portability

Omarchy/Linux is the first integration body, not the definition of Lychnos.

The platform-independent Rust core must not require Omarchy, PipeWire, GTK, llama.cpp, Qwen, Piper, Whisper, or another body-specific technology.

Architectural targets include Linux, Windows, macOS, Android, iOS, and future Pocket/embedded Lychnos.

Different bodies may use different local inference, STT, TTS, UI, acceleration, and packaging adapters while preserving the same Lychnos contracts.

## Intelligence

A free local brain is the no-subscription baseline.

The core does not define one universal model.

Local intelligence is capability-aware and may use Tiny / Compact / Standard / Large resource classes according to the current body.

The current Omarchy/X1 Pro llama.cpp + Qwen configuration is one body-specific adapter, not a universal dependency.

The intelligence-router direction is:

```text
LocalBrain       — free/local baseline
AccountBridge    — optional supported account/subscription-backed enhancement
ApiByok          — optional user-supplied API path only
```

Paid API access must never be required for baseline Lychnos.

Account bridges must use provider-supported authentication and integration mechanisms.

## Memory

Authoritative memory belongs to Lychnos, not to an intelligence provider.

Changing models/providers must not erase the companion.

Short-term model/session context is not durable Lychnos memory.

Persistent local memory must eventually support shared identity across bodies. Synchronization, pairing, encryption, conflict resolution, and offline reconciliation remain separate design work.

## Character and Initiative

Lychnos should feel like a companion rather than a command box.

Character/persona is Lychnos-owned and provider-independent.

Lychnos may reason, explain, notice, suggest, ask, and form bounded proactive observations.

Proactive initiative and system execution are separate capabilities.

A proactive thought may surface text or speech; it does not grant itself permission to act.

Initiative should be useful, contextual, concise, and non-repetitive.

Lychnos should not speak merely to appear alive.

Meaningful context should be the reason to consider initiative. User idle time is an interruption/appropriateness guard, not by itself a reason to invent something to say.

Game Mode and Disabled mode suppress proactive AI initiative.

## Observation and Ambient Awareness

Lychnos is intended to live alongside the machine, not merely be a floating chat client.

The long-term distinguishing experience remains:

> “Terminal 3 hit a snag. I think I know why. Want me to fix it?”

That requires explicitly permitted collectors and contextual awareness.

Future development must balance both sides:

```text
companion intelligence        ambient awareness
persona                       collectors
memory                        normalized events
voice                         diagnostics
initiative                    system context
          \                  /
                 Lychnos
```

Do not allow interaction/AI work to permanently crowd out collectors/context work.

## User Authority

The user remains the authority for meaningful system actions.

The AI model is never the final authority for host changes.

The intended action path is:

```text
observation
→ reasoning/analysis
→ structured action proposal
→ permission/runtime checks
→ user approval where required
→ restricted executor
→ security audit
```

Lychnos itself remains unprivileged by default. Any future privileged helper must be narrow, separately reviewable, and outside the intelligence-provider boundary.

## Safety Modes

Normal, Game Mode, and Disabled are core runtime concepts.

Disabled must be a real runtime invariant, not merely hidden UI. Re-enabling from Disabled must be explicit.

Game Mode exists to protect gaming/streaming performance, latency, thermals, and anti-cheat-sensitive workloads.

As heavier local intelligence, voice, and monitoring become real, Game Mode should be able to suspend or unload resource-heavy components rather than merely telling them not to speak.

## Local First and Privacy

Configuration, authoritative memory, event processing, and security records are local by default.

Cloud/account providers are optional and should receive only the context required for an explicitly enabled request.

No core telemetry is required.

Microphone use must remain explicit, visible, and configurable.

Wake-word processing should be local where practical, with no network request required merely to detect activation.

## Voice and Interaction

Typed input, Push-to-Talk, Wake Word, and Voice Session share one normalized conversation path.

The intended mature voice flow is:

```text
wake/PTT/session
→ local capture
→ STT
→ Lychnos intelligence/persona/context
→ response
→ text, voice, or both
```

The current voice model and engine are replaceable. Voice identity belongs to Lychnos, not Piper or a particular model file.

## Visual Presence

The canonical visual reference is:

`assets/canon/lychnos-body-v1.webp`

Desktop presence, phone presence, and future physical Pocket body are expressions of the same companion identity.

Presentation surfaces must not contain system-action authority.

## Security and Auditability

Diagnostic logs and the security audit are separate concepts.

The security audit is mandatory in the architecture.

Native and imported data must be normalized and treated cautiously at adapter boundaries.

Intelligence providers may consume context, but policy and host-change authority remain outside the model boundary.

## Open Development

Lychnos is intended to be publicly developed with understandable architecture, ADRs, reproducible builds, automated quality gates, and a contributor path that does not depend on hidden infrastructure.

## Development Discipline

Platform-independent foundations come first.

Body-specific behavior belongs behind explicit adapters.

Do not prematurely lock one AI model, inference runtime, database, UI toolkit, event bus, synchronization transport, or voice engine unless a concrete body-specific adapter needs one. When that happens, document it as an adapter rather than core canon.

## Continuity

The repository is the source of truth across chat and session limits.

Every future development session must begin with `START-HERE.md` and `DEVELOPMENT-HANDOFF.md`.

Every meaningful session must leave enough documentation that another session can continue without relying on hidden chat history.
