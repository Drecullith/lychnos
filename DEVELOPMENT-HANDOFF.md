# Project Lychnos — Development Handoff

Last updated: 2026-09-26

Purpose: exact operational handoff for the next development chat/session. Read `START-HERE.md` and `PROJECT-CANON.md` before acting.

## Current Git / Milestones

Branch: `feat/phase2-approval-flow`

Latest implementation milestones:

- `4d528cf Add first read-only ambient collector`
- `0aa5c4b Add explicit durable memory recall`
- `486a945 Add first durable local memory adapter`
- `2df1cce Add context-triggered live initiative`
- `63fe5f4 Add repository continuity handoff`
- `5204b25 Add first capability-aware local brain adapter`
- `5159bfb Add cross-device intelligence routing`
- `9f27f85 Add bounded Lychnos initiative model`
- `11e2c81 Add local Lychnos speech output`
- `b5db759 Add local push-to-talk speech recognition`

Always run `git status --short --branch` before changing code. At this handoff point all implementation slices through ADR 0051 and the handoff refresh are committed and pushed; the working tree should be clean unless a later session has started new work.

## GitHub Repository Hygiene Audit — COMPLETE

A full GitHub/repository continuity audit was completed.

Verified:

- remote branches: `main`, `feat/core-bootstrap`, `feat/phase2-approval-flow`;
- no older duplicate handoff/handover/canon/continuity files in remote branch trees;
- no hidden renamed/deleted competing handoff convention in Git history;
- no exact duplicate Markdown documents;
- continuity files have distinct roles rather than duplicated bodies;
- ADR numbers, titles, and normalized bodies have no duplicates;
- ADR numbering is contiguous through 0051;
- GitHub issues contain no competing project-state convention;
- historical Phase 1 PR is not a second source of current truth;
- continuity docs and recent ADRs were verified through the GitHub connector.

Single continuity system:

- `START-HERE.md`
- `PROJECT-CANON.md`
- `DEVELOPMENT-HANDOFF.md`
- `docs/CONTINUITY-PROTOCOL.md`

Do not create parallel session-summary/handover files unless this design is deliberately reopened.

## Canon Audit — Agreed Direction

The project remains aligned with the original Lychnos vision.

Locked direction:

- free/local intelligence is the no-subscription baseline;
- no universal model/runtime; bodies choose capability-appropriate adapters;
- AccountBridge is optional where providers expose supported account/subscription integration;
- BYOK API remains optional, never baseline-required;
- Lychnos owns identity, persona, memory, permissions, initiative, and security semantics;
- same Lychnos identity across desktop, phone, and Pocket bodies;
- Omarchy is the first adapter body, not core canon;
- initiative can speak/suggest but never grants host-action authority;
- meaningful host changes remain behind permission/approval/executor/audit boundaries;
- local-first privacy and wake-word direction remain locked.

Course corrections already implemented:

- initiative is context-triggered; silence alone is never a reason to invent something to say;
- durable memory moved up in priority and is now live;
- first explicit capture/filtered recall policy is live;
- first real read-only ambient collector is live;
- roadmap now reflects early prototype validation of voice/UI/local AI.

Still important:

- do not let Lychnos become merely a talking orb;
- ambient context must expand carefully alongside intelligence;
- Game Mode must eventually suspend/unload expensive inference/voice/collector work based on measurements;
- degraded/fallback intelligence state should become visible in the shell.

## LIVE-VERIFIED — Development Body

Machine:

- Minisforum AI X1 Pro
- AMD Ryzen AI 9 HX 370
- Radeon 890M
- about 60 GiB RAM
- Omarchy/Linux

Current installed Lychnos:

- runtime running;
- shell running;
- normal/default persistent memory store empty after disposable tests;
- first real ambient collector installed and polling healthy user-service state.

Body-specific intelligence:

- llama.cpp
- `Qwen/Qwen3-8B-GGUF:Q4_K_M`
- Large class
- context 8192
- loopback endpoint 127.0.0.1:18181
- model-specific `/no_think` remains adapter-local.

Voice:

- local Whisper multilingual-base STT;
- Piper `en_GB-alan-medium` TTS;
- X1 Pro input port `analog-input-mic`, input level 30%.

## LIVE-VERIFIED — Voice / Conversation

Working path:

`Push-to-Talk -> PipeWire -> Whisper -> ConversationContext -> LocalBrain -> shell text -> Piper speech`

Verified:

- PTT lifecycle works;
- local transcription works;
- pure non-speech labels such as `(upbeat music)` are filtered;
- LocalBrain produces real non-mock replies;
- spoken replies play locally;
- typed and PTT use the same provider-neutral conversation path.

Wake word is not implemented yet.

## COMMITTED — Intelligence / Initiative

Capability-aware core includes:

- `BodyPlatform`
- `DeviceCapabilityProfile`
- `LocalBrainClass::{Tiny, Compact, Standard, Large}`
- `LocalModelManifest`
- `LocalBrainSelectionPolicy`
- `IntelligenceSource::{Local, AccountBridge, ApiByok}`
- `IntelligenceRouter`

ADR 0048 context-triggered initiative:

- transient six-turn/twelve-message session context;
- session context is not durable memory;
- explicit `InitiativeTrigger`;
- user activity only delays interruption;
- meaningful context opens consideration;
- unchanged context considered at most once;
- Game Mode/Disabled suppress initiative;
- no host-action authority.

## COMMITTED / LIVE-VERIFIED — Durable Memory

ADR 0049 first durable adapter:

- provider-independent `MemoryStore` remains core boundary;
- first desktop adapter is versioned JSON snapshot;
- default path `~/.local/share/lychnos/memory-v1.json`;
- missing file means empty store;
- deterministic ordering;
- schema validation;
- atomic publish on current Linux body;
- Unix owner-only permissions;
- rollback on failed persistence;
- preserves provenance/sensitivity/confidence/device/sync metadata.

ADR 0050 explicit capture + filtered recall:

- only explicit `Remember ...` directives are durably captured;
- ordinary conversation is not silently promoted into memory;
- exact repeats deduplicated;
- obvious credential/secret-like memory requests refused while at-rest encryption is absent;
- local recall filters to non-tombstoned Standard-sensitivity identity memories;
- bounded lexical relevance: max 5 memories / 1500 total chars / 500 chars each;
- broad explicit memory questions can recall bounded recent identity memories;
- `ConversationProvider` receives already-filtered `ConversationContext`, never direct MemoryStore access;
- recalled memories are explicitly framed as DATA, not instructions;
- typed and PTT share the same memory policy.

Durable restart test:

1. used disposable `LYCHNOS_MEMORY_PATH`;
2. stored explicit test phrase `cobalt lantern`;
3. confirmed one durable record;
4. restarted Lychnos fully;
5. startup reopened one record;
6. asked what Lychnos remembered;
7. LocalBrain correctly recalled `cobalt lantern` from durable storage.

Disposable test state was removed afterward. Real/default memory store was restored and verified empty.

## COMMITTED / LIVE-VERIFIED — First Ambient Collector (ADR 0051)

First real host observation adapter:

- failed systemd user-service health only;
- body-specific Omarchy runtime adapter;
- invokes `systemctl --user --failed --no-legend --plain --no-pager` read-only;
- maximum 15-second poll cadence;
- validates/parses unit names only;
- healthy unchanged baseline emits no event;
- existing startup failure emits one current-condition event;
- changed failure sets emit normalized Error events;
- full recovery emits normalized Info event;
- same unchanged state does not repeat events/model calls.

Privacy boundary:

- does not read terminal content;
- does not read process arguments;
- does not read journal text;
- does not read files, browser content, or keystrokes.

Foundation path:

`systemd user state -> Omarchy adapter -> normalized Event -> FoundationRuntime collector/event bus -> mock analyzer/audit simulation`

Meaningful ambient events also create bounded `InitiativeObservation` data under a `DiagnosticChange` trigger.

`InitiativeObservation` is framed to the model as contextual DATA, not instructions.

A real ambient observation can open initiative even without prior chat; silence alone still cannot.

Game Mode/Disabled suppression remains core-owned because `FoundationRuntime::collect_once` does not poll collectors when background work is disallowed.

Live healthy-body test:

- X1 Pro user manager reported `running`;
- zero failed user units;
- collector produced no ambient event (correct);
- no collector error;
- runtime and shell remained running after multiple real poll intervals.

## Quality Gate

Latest strict gate:

- core: 177 tests passed;
- runtime: 32 tests passed;
- shell: 5 tests passed;
- strict Clippy clean.

ADR hygiene after 0051:

- 51 ADRs;
- contiguous 0001 through 0051;
- no duplicate ADR number/title/body.

## Important Remaining Gaps

Memory:

- user-facing memory inspection/listing controls;
- forget/edit/tombstone controls;
- automatic/inferred memory policy;
- semantic retrieval and consolidation;
- encrypted-at-rest sensitive memory;
- AccountBridge/cloud memory disclosure policy;
- cross-device synchronization/pairing/conflict resolution;
- final globally unique device/memory IDs.

Ambient:

- broader service/system context;
- terminal awareness with explicit scope/privacy design;
- journal context;
- hardware telemetry;
- automatic Game Mode detection.

Interaction/intelligence:

- wake-word activation;
- account-backed provider bridges;
- provider/fallback status UI;
- intelligence/voice/persona settings;
- speech interruption/ducking/output-device controls;
- additional Windows/macOS/Android/iOS/Pocket LocalBrain adapters.

Safety/performance:

- persistent security audit backend/integrity;
- Game Mode measurements and heavy-component unload/suspend policy;
- real restricted host execution only after current safety boundaries are ready.

## Next Safe Development Direction

Recommended next sequence:

1. add explicit memory inspection + forget/tombstone controls so durable memory stays visibly user-owned and reversible;
2. expand ambient awareness with another low-privacy-risk read-only context source before terminal-content monitoring;
3. connect meaningful collector events into richer initiative reasoning without widening authority;
4. begin Game Mode resource measurements before adding heavier always-on intelligence;
5. keep wake-word/account bridges on the existing provider-neutral boundaries.

A good next ambient candidate is broader user-service/system health metadata or selected hardware telemetry. Terminal content should wait for explicit scoping/privacy rules.

## Resume Verification

Run:

```bash
cd /home/drec/Work/lychnos
git status --short --branch
git log -15 --oneline
~/.local/bin/lychnos status
tail -n 60 ~/.local/state/lychnos/runtime.log
```

Then read:

- `START-HERE.md`
- `PROJECT-CANON.md`
- `DEVELOPMENT-HANDOFF.md`
- `CURRENT-STATE.md`
- ADRs 0048–0051.

## Documentation Rule

Keep this as the single development handoff. Update it at the end of every meaningful slice; do not create parallel handover/session-summary files.
