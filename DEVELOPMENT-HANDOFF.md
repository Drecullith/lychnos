# Project Lychnos — Development Handoff

Last updated: 2026-09-26

Purpose: exact operational handoff for the next development chat/session.

Read START-HERE.md and PROJECT-CANON.md before acting on this file.

## Current Git State

Branch: feat/phase2-approval-flow

Current state after the continuity documentation commit:

- ahead of origin/feat/phase2-approval-flow by 13 commits;
- the current HEAD is the continuity documentation milestone, titled `Add repository continuity handoff`;
- the only remaining dirty files are the intentionally preserved WIP initiative/session-context files listed below.

Recent committed milestones before the continuity HEAD:

- 5204b25 Add first capability-aware local brain adapter
- 5159bfb Add cross-device intelligence routing
- 9f27f85 Add bounded Lychnos initiative model
- 11e2c81 Add local Lychnos speech output
- b5db759 Add local push-to-talk speech recognition
- b7e4fd6 Add normalized microphone discovery
- da4b261 Add typed interaction and Lychnos persona runtime
- 6e099b6 Add safe pending approval controls
- f129d30 Add Omarchy install and launch flow
- 90e4e44 Add click-through Ghost Mode
- cbb2722 Persist shell preferences
- b26f049 Connect shell to live presentation state
- ef5bea0 Add Omarchy minimize and restore integration
- 21c1fc9 Add shell context menu controls
- 3011d63 Add canonical expression overlays to shell

## WIP-UNCOMMITTED — Preserve Carefully

The working tree contains an unfinished session-context/live-initiative slice:

- M ARCHITECTURE.md
- M crates/lychnos-runtime/src/local_brain.rs
- M crates/lychnos-runtime/src/main.rs
- M prototypes/lychnos-shell/src/main.rs
- ?? docs/decisions/0048-session-context-and-live-initiative-scheduler.md

Do not discard these files without reviewing the diff.

Current WIP behavior:

- bounded six-turn/twelve-message session-local conversation history;
- transient context reused by later conversation turns;
- transient context reused by initiative proposals;
- runtime initiative scheduling;
- 60-second user-quiet guard;
- unchanged context considered at most once;
- requires Normal mode and no pending approval;
- surfaced initiative uses the normal response/TTS path;
- surfaced initiative is added to transient session context;
- shell can display initiative responses without taking keyboard focus.

### Canon-audit conclusion on this WIP

The scheduler should not be locked exactly as written.

Agreed direction:

- meaningful context or event change should be the reason Lychnos considers initiative;
- user idle time should only be an interruption/appropriateness guard;
- avoid “60 seconds passed, find something to say” behavior;
- revise ADR 0048 and scheduler logic before committing this slice.

The transient session-history part remains directionally good as long as it stays separate from durable Lychnos memory.

## COMMITTED — Intelligence Architecture

The platform-independent core now has capability-aware intelligence routing.

Key concepts:

- BodyPlatform
- DeviceCapabilityProfile
- LocalBrainClass: Tiny / Compact / Standard / Large
- LocalModelManifest
- LocalBrainSelectionPolicy
- IntelligenceSource: Local / AccountBridge / ApiByok
- IntelligenceRouter

Locked direction:

- free/local intelligence is the no-subscription baseline;
- no one model or inference runtime is universal Lychnos;
- desktops, phones, macOS, Windows, Android, iOS, and Pocket bodies may use different adapters;
- AccountBridge is optional where a provider exposes a supported account/subscription mechanism;
- BYOK API support may exist but is optional and never required for baseline Lychnos;
- identity, memory, persona, permissions, and initiative must survive provider/model changes.

## COMMITTED — First LocalBrain Adapter

ADR 0047 establishes the first Omarchy/Linux LocalBrain adapter.

Current body-specific implementation:

- llama.cpp local server;
- loopback-only endpoint;
- ephemeral local authentication token;
- child process owned by the Lychnos runtime;
- ConversationProvider implemented by LocalBrain;
- InitiativeProvider implemented by LocalBrain;
- deterministic mock provider remains fallback;
- model-specific prompt suffix stays outside the core;
- model-internal think blocks are removed from visible replies.

Current X1 Pro body configuration:

- runtime: llama.cpp
- model: Qwen/Qwen3-8B-GGUF:Q4_K_M
- class: Large
- context: 8192
- endpoint: 127.0.0.1:18181

This configuration is not universal canon. It is only the current Omarchy/X1 Pro body adapter.

Reference desktop classes currently used by the installer:

- Tiny: Qwen3 0.6B Q8_0
- Compact: Qwen3 1.7B Q8_0
- Standard: Qwen3 4B Q4_K_M
- Large: Qwen3 8B Q4_K_M

Other operating systems, phones, and Pocket bodies may use different runtimes/models behind the same core contracts.

## LIVE-VERIFIED — Current Development Body

Machine:

- Minisforum AI X1 Pro
- AMD Ryzen AI 9 HX 370
- Radeon 890M
- about 60 GiB RAM
- Omarchy/Linux

Current installed Lychnos state:

- Runtime: running
- Shell: running

Runtime startup currently confirms:

- Speech-to-text ready: whisper.cpp, multilingual base model
- Text-to-speech ready: Piper, en_GB-alan-medium
- Local brain ready: llama.cpp, Qwen/Qwen3-8B-GGUF:Q4_K_M
- Persona: Lychnos

Machine-local brain config lives at ~/.config/lychnos/brain.env.

Current values:

- LYCHNOS_LOCAL_BRAIN_ENABLED=1
- LYCHNOS_LOCAL_BRAIN_RUNTIME=/home/drec/.local/bin/llama
- LYCHNOS_LOCAL_BRAIN_MODEL=Qwen/Qwen3-8B-GGUF:Q4_K_M
- LYCHNOS_LOCAL_BRAIN_PORT=18181
- LYCHNOS_LOCAL_BRAIN_CONTEXT=8192
- LYCHNOS_LOCAL_BRAIN_USER_SUFFIX=/no_think

Machine-local voice config lives at ~/.config/lychnos/voice.env.

Current values:

- LYCHNOS_AUDIO_SOURCE_PORT=analog-input-mic
- LYCHNOS_AUDIO_INPUT_VOLUME_PERCENT=30

## LIVE-VERIFIED — Interaction and Voice

Working physical flow:

Push-to-Talk
→ external microphone
→ PipeWire capture
→ local Whisper STT
→ LocalBrain conversation
→ shell text response
→ local Piper TTS
→ PipeWire playback

Verified behaviors:

- PTT press/release works after gesture and recorder-lifecycle fixes;
- runtime acknowledges microphone state instead of the shell guessing;
- external microphone route is explicitly configured on the X1 Pro;
- local Whisper produces real transcripts;
- LocalBrain produces non-mock conversational replies;
- Piper produces audible spoken replies;
- the baseline voice was positively received, though it sounds slightly unusual;
- temporary voice files are intended to remain ephemeral;
- pure non-speech Whisper labels such as “upbeat music” are filtered instead of becoming literal user text.

Typed, Push-to-Talk, Wake Word, and Voice Session already share one normalized interaction vocabulary.

Wake-word activation is not implemented yet.

## COMMITTED — Character and Initiative Boundary

Canonical Lychnos persona includes:

- warm;
- observant;
- concise;
- curious;
- dry-witted;
- calm under pressure;
- companion-like conversational style;
- initiative style that prefers useful silence over chatter.

The initiative architecture deliberately separates:

thought or observation
→ maybe surface as text/speech

from:

structured proposed host change
→ policy/runtime checks
→ approval where required
→ restricted host adapter

A proactive thought does not grant system authority.

Game Mode and Disabled suppress initiative.

## CANON AUDIT — Agreed Conclusions

A full canon audit was performed immediately before this handoff.

Conclusion: the project is still following the original vision. Several newer architectural choices are improvements rather than drift.

Keep:

1. capability-aware LocalBrain architecture;
2. free/local no-subscription baseline;
3. optional AccountBridge plus optional BYOK routing;
4. provider-independent identity/persona/memory/permissions;
5. bounded initiative separate from host actions;
6. Omarchy as first adapter, not the core;
7. existing safety, approval, and audit boundaries;
8. one Lychnos identity across desktop, phone, and Pocket bodies.

Course corrections agreed:

### Persistent memory moves higher in priority

Transient session context is useful but is not Lychnos memory.

Character without continuity is insufficient.

Cross-body Lychnos ultimately depends on durable local memory that belongs to Lychnos rather than whichever brain is active.

### Initiative becomes context-triggered

Meaningful context/event change should open an initiative consideration window.

Idle time should answer “is this a good time to interrupt?” rather than “should I invent something to say?”

### Ambient awareness must catch up

Do not let Lychnos become only a talking AI orb.

The original distinguishing experience remains environmental awareness through explicitly permitted collectors and normalized context.

### Game Mode must eventually manage heavy components

Suppressing speech is not enough.

Local inference, model residency, wake processing, collectors, diagnostics, and other heavier components may need suspension or unload based on measured gaming/streaming impact.

### Brain fallback should be visible

If LocalBrain fails and the deterministic mock provider is used, the shell should make the degraded/fallback intelligence state visible.

### Roadmap sequencing needs refresh

UI, voice, local AI, approval UI, and provider work were pulled forward from the old later-phase plan.

That was useful prototype validation, not abandonment of the safety-first development rule.

ROADMAP.md should be updated to describe the actual sequence.

## Important Gaps

Near-term gaps include:

- persistent local memory backend;
- memory retrieval/context assembly for conversation and initiative;
- first real read-only ambient collectors;
- terminal/journal/service context;
- portable memory synchronization;
- wake-word activation;
- additional LocalBrain adapters for Windows, macOS, Android, iOS, and Pocket;
- portable model catalog mappings for non-llama runtimes;
- visible intelligence-provider/fallback status in the shell;
- user-facing intelligence/voice/persona settings;
- Game Mode performance measurements and heavy-component suspension;
- persistent security-audit backend;
- automatic Game Mode detection;
- restricted real host-action adapters only after the safety boundaries are ready.

## Immediate Next Safe Step

Before adding another major capability:

1. preserve the current uncommitted session-context/initiative work;
2. revise ADR 0048 and scheduler semantics so meaningful context is the trigger and user idleness is only an interruption guard;
3. refresh ROADMAP.md to reflect the actual development sequence;
4. move persistent local Lychnos memory into the near-term plan;
5. choose the next implementation slice while balancing:
   - durable local memory, and
   - first real read-only ambient/context collector.

Do not start another LocalBrain/model-selection redesign. The capability-aware intelligence architecture is already the agreed direction.

## Resume Verification Commands

At the start of a future development chat:

```bash
cd /home/drec/Work/lychnos
git status --short --branch
git log -15 --oneline
~/.local/bin/lychnos status
tail -n 50 ~/.local/state/lychnos/runtime.log
```

Then read:

```text
START-HERE.md
PROJECT-CANON.md
DEVELOPMENT-HANDOFF.md
CURRENT-STATE.md
relevant recent ADRs
```

For the current WIP, inspect the diff before doing anything else:

```bash
git diff -- ARCHITECTURE.md \
  crates/lychnos-runtime/src/local_brain.rs \
  crates/lychnos-runtime/src/main.rs \
  prototypes/lychnos-shell/src/main.rs
```

Also read:

```text
docs/decisions/0048-session-context-and-live-initiative-scheduler.md
```

## Documentation Rule Going Forward

At the end of every meaningful development session:

- update this handoff;
- update CURRENT-STATE.md and ARCHITECTURE.md when behavior changed;
- update ROADMAP.md when sequencing changed;
- create or revise ADRs for lasting architectural choices;
- record real/manual hardware tests;
- record dirty and untracked files;
- record the exact next safe step.

If something would be painful to reconstruct after a chat cap, document it before ending the session.
