# Project Lychnos — Development Handoff

Last updated: 2026-09-26

Purpose: exact operational handoff for the next development chat/session. Read `START-HERE.md` and `PROJECT-CANON.md` before acting.

## Current Git / Milestones

Branch: `feat/phase2-approval-flow`

Latest implementation milestones:

- ADR 0053 Voice V2 live perception/session architecture — physically verified ambient conversation milestone
- ADR 0052 Conversation output modes, chat history, and live companion activity
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

Always run `git status --short --branch` before changing code. ADR 0053 is the physically verified Voice V2 milestone; preserve the working wake/session architecture and treat later TTS/language/platform work as replaceable adapters around it.

## GitHub Repository Hygiene Audit — COMPLETE

A full GitHub/repository continuity audit was completed.

Verified:

- remote branches: `main`, `feat/core-bootstrap`, `feat/phase2-approval-flow`;
- no older duplicate handoff/handover/canon/continuity files in remote branch trees;
- no hidden renamed/deleted competing handoff convention in Git history;
- no exact duplicate Markdown documents;
- continuity files have distinct roles rather than duplicated bodies;
- ADR numbers, titles, and normalized bodies have no duplicates;
- ADR numbering is contiguous through 0053;
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
- dedicated Voice V2 perception process (`lychnos-perception-omarchy`);
- sherpa-onnx open-vocabulary KWS for `Lychnos`;
- WebRTC VAD for speech onset/end-of-turn;
- sherpa speaker embeddings for ephemeral per-session speaker verification;
- X1 Pro input port `analog-input-mic`; current calibration/testing raised source volume from 30% to 80%.

## LIVE-VERIFIED — Voice / Conversation

### Voice V2 architecture

Current preferred ambient path:

`PipeWire microphone -> sherpa-onnx keyword spotting -> WebRTC VAD -> Whisper STT -> ConversationContext -> Qwen LocalBrain -> Piper TTS -> bounded follow-up session`

Core ownership:

- `crates/lychnos-core/src/perception.rs` owns the deterministic live-session state machine;
- states: `WakeArmed -> Listening -> Thinking -> Speaking -> FollowUp -> Listening/WakeArmed`;
- invalid transitions are rejected rather than guessed by scattered booleans/timers;
- Audio and Vision are first-class perception modalities for future Pocket/camera work.

Body adapter:

- `crates/lychnos-perception-omarchy` is a separate process;
- it owns microphone/KWS/VAD/speaker-verification mechanics only;
- runtime owns AI/persona/session authority;
- perception failure must not take down the main runtime;
- Voice V1 wake listener is disabled whenever Voice V2 starts successfully, so there is one ambient microphone owner.

Wake detection:

- sherpa-onnx Zipformer KWS;
- current wake phrase: `Lychnos`;
- phonetic variants cover observed forms such as Lich/Leek/Lick Noss/Nuss;
- live natural-pronunciation detection verified;
- natural inline wake is physically verified: `So, Lychnos, what are we doing today?` woke the agent and preserved the rest of the sentence as the user request;
- wake -> Listening -> utterance finalized cleanly after 2.7s in calibration;
- Whisper is not used to decide whether the wake phrase occurred.

Turn endpointing:

- WebRTC VAD handles speech boundaries;
- this eliminated the repeated 30-second runaway captures from Voice V1;
- Whisper now performs transcription only.

Natural hands-free session:

- wake phrase opens the session;
- Lychnos gives acknowledgement + spoken reply;
- after speech playback completes, a bounded follow-up window opens;
- user can continue without repeating the wake phrase;
- session returns to wake-required mode after expiry or an explicit close phrase;
- close phrases include `stand down`, `stop listening`, `go idle`, `that's enough for now`, `we can stop for now`, goodbye/later phrases, and prior return-to-work/fixing phrases;
- `Thank you brother` alone intentionally does not close the session.

Chat/UI mirroring:

- Voice V2 transcripts appear as `You` in `CHAT · LOCAL BRAIN`;
- matching Lychnos replies appear in the same bounded chat history;
- shell recognizes `voice-v2-*` request/capture IDs as hands-free turns.

Speaker isolation / anti-video behavior:

- follow-up speech requires a short continuous VAD onset;
- the wake turn creates an **ephemeral in-RAM speaker embedding** for the person who opened the session;
- no permanent voiceprint is written to Lychnos memory;
- no-wake follow-ups are compared against that session speaker;
- initial cosine threshold: `0.35` (configurable);
- calibration: same-user split about `0.494`; user-vs-Piper about `0.1375`;
- rejected non-session speakers return to FollowUp without reaching Whisper/Qwen;
- fingerprint clears when the session expires/returns to WakeArmed/Disabled;
- normal wake -> answer -> follow-up -> stop behavior is physically verified;
- deliberate video-speaker rejection test is **physically verified**: user follow-ups were accepted around cosine similarity `0.375–0.461`, while video/background voices were rejected around `0.10–0.29` before reaching Whisper/Qwen.

Provider/runtime:

- expected label is `local llama.cpp · Qwen/Qwen3-8B-GGUF:Q4_K_M`, not `local-mock`;
- initiative is now hard-blocked unless companion activity is `Idle`; this fixes an intermittent bug where a long spoken reply could be overwritten from `Speaking` to `Idle` before Voice V2 re-armed follow-up;
- Voice V2 now exits when the runtime control pipe closes, so perception cannot remain as an orphan competing for the microphone after a runtime restart;
- runtime now handles SIGTERM/Ctrl-C gracefully and drops both managed Qwen and perception children; lifecycle proof verified zero runtime/llama/perception processes remain after SIGTERM;
- a stale unknown llama server on port 18181 still causes deliberate fallback to mock, but normal graceful Lychnos shutdown no longer leaves that stale process behind;
- current runtime launches/manages its perception child.

### Older PTT path remains available

`Push-to-Talk -> PipeWire -> Whisper -> ConversationContext -> LocalBrain -> shell text -> Piper speech`

PTT lifecycle, local transcription, non-speech filtering, typed/voice normalized conversation path, activity projection, and Voice ON/OFF preference remain intact.

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

Latest full Voice V2 milestone gate:

- workspace core: **188 tests passed**;
- runtime: **47 tests passed**;
- perception/CLI build and test targets passed;
- shell: **7 tests passed**;
- workspace strict Clippy: clean;
- shell strict Clippy: clean;
- workspace release build: clean;
- shell release build: clean;
- `git diff --check`: clean.

ADR hygiene after 0053:

- 53 ADRs;
- contiguous 0001 through 0053;
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

- monitor/tune Voice V2 follow-up speaker isolation only if new real-world false accepts/rejects appear;
- expose user-facing wake-name / agent / voice / provider settings;
- add Voice mode settings (Wake / PTT / Both / Off);
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

1. Preserve the currently working sherpa KWS wake path; do not redesign wake detection again unless evidence requires it.
2. Keep the current follow-up speaker threshold unless new real-world evidence shows false accepts/rejects; the anti-video test is now physically verified.
3. Run full tests + strict Clippy, then commit/push the complete Voice V2 slice as one coherent milestone.
4. Add user-facing settings for agent/provider, persona/display name, voice, and wake call. The configured agent name should be able to drive the wake phrase where supported.
5. Add explicit Voice mode settings: Wake only / PTT only / Both / Voice off.
6. Then return to memory inspection + forget/tombstone controls and broader low-risk ambient context.
7. Keep AccountBridge and host-changing actions on existing provider-neutral permission/audit boundaries.

### Future portable perception

The roadmap/canon now explicitly treat camera/vision as a first-class future perception modality for Pocket Lychnos. The intended portable body can eventually hear and see ambient context while remaining local-first, visible/permission-bounded, and without automatically hoarding raw video.

## Resume Verification

Run:

```bash
cd /home/drec/Work/lychnos
git status --short --branch
git log -15 --oneline
~/.local/bin/lychnos status
tail -n 100 ~/.local/state/lychnos/runtime.log
tail -n 100 ~/.local/state/lychnos/perception-v2.log
```

Then read:

- `START-HERE.md`
- `PROJECT-CANON.md`
- `DEVELOPMENT-HANDOFF.md`
- `CURRENT-STATE.md`
- ADRs 0048–0053.

## Documentation Rule

Keep this as the single development handoff. Update it at the end of every meaningful slice; do not create parallel handover/session-summary files.
