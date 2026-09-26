# Project Lychnos — Development Handoff

Last updated: 2026-09-26

Purpose: exact operational handoff for the next development chat/session. Read START-HERE.md and PROJECT-CANON.md first.

## Current Git State

Branch: `feat/phase2-approval-flow`

Remote GitHub branch is synchronized through:

- `486a945 Add first durable local memory adapter`
- `2df1cce Add context-triggered live initiative`
- `63fe5f4 Add repository continuity handoff`
- `5204b25 Add first capability-aware local brain adapter`
- `5159bfb Add cross-device intelligence routing`
- `9f27f85 Add bounded Lychnos initiative model`
- `11e2c81 Add local Lychnos speech output`
- `b5db759 Add local push-to-talk speech recognition`

Current WIP-UNCOMMITTED files:

- `ARCHITECTURE.md`
- `CURRENT-STATE.md`
- `ROADMAP.md`
- `crates/lychnos-core/src/interaction.rs`
- `crates/lychnos-runtime/src/local_brain.rs`
- `crates/lychnos-runtime/src/main.rs`
- `crates/lychnos-runtime/src/memory_store.rs`
- `crates/lychnos-runtime/src/memory_context.rs` (new)
- `docs/decisions/0050-explicit-memory-capture-and-filtered-recall.md` (new)

Preserve this WIP carefully. It is the explicit-memory capture + filtered-recall slice described below.

## GitHub Repository Hygiene Audit — COMPLETE

A full GitHub/repo documentation audit was completed before continuing development.

Verified:

- remote branches are `main`, `feat/core-bootstrap`, and `feat/phase2-approval-flow`;
- no older duplicate handoff/handover/canon/continuity files exist in current remote trees;
- Git history contains no hidden renamed/deleted competing handoff convention;
- no exact duplicate Markdown documents were found;
- semantic-overlap scan showed the continuity documents have distinct roles rather than copied content;
- ADR numbers/titles/bodies had no duplicates;
- ADR numbering was contiguous through the current series;
- GitHub issues do not contain a competing project-state/handoff convention;
- the historical Phase 1 PR is not a second source of current truth;
- continuity files are present on the GitHub feature branch.

Result: the repository has one continuity system only:

- `START-HERE.md`
- `PROJECT-CANON.md`
- `DEVELOPMENT-HANDOFF.md`
- `docs/CONTINUITY-PROTOCOL.md`

Do not create additional session-summary/handover files unless this continuity design is deliberately reopened.

## Canon Audit — Agreed Direction

The project is still following the original Lychnos vision. Several newer choices improve it.

Locked/keep:

- free/local intelligence as the no-subscription baseline;
- capability-aware LocalBrain routing rather than one universal model;
- optional supported AccountBridge integrations and optional BYOK only;
- Lychnos-owned identity/persona/memory/permissions/initiative;
- same Lychnos across desktop, phone, and Pocket bodies;
- Omarchy as first body adapter, never the core;
- bounded initiative separate from host-action authority;
- explicit approval/runtime/audit boundaries;
- local-first voice and wake-word direction.

Course corrections already acted on:

- initiative is now context-triggered; silence alone does not generate thoughts;
- persistent Lychnos-owned memory moved higher in priority;
- roadmap now acknowledges voice/UI/local AI were deliberately pulled forward for prototype validation;
- next large product balance is durable memory plus real ambient/read-only context.

Still important:

- do not let Lychnos become merely a talking orb;
- ambient collectors/system context must catch up;
- Game Mode must eventually suspend/unload heavy inference/voice/collector work based on measured impact;
- provider fallback/degraded brain state should become visible in the shell.

## LIVE-VERIFIED — Development Body

Machine:

- Minisforum AI X1 Pro
- AMD Ryzen AI 9 HX 370
- Radeon 890M
- about 60 GiB RAM
- Omarchy/Linux

Current normal installed state:

- runtime running;
- shell running;
- real/default Lychnos memory store restored to empty after disposable tests.

Current body-specific brain config:

- llama.cpp runtime
- `Qwen/Qwen3-8B-GGUF:Q4_K_M`
- Large class
- 8192 context
- loopback endpoint 127.0.0.1:18181
- `/no_think` suffix stays adapter-local.

Current voice stack:

- local Whisper STT, multilingual base model;
- Piper TTS, `en_GB-alan-medium` baseline voice;
- X1 Pro microphone source port `analog-input-mic`, input level 30%.

## LIVE-VERIFIED — Interaction and Voice

Working physical flow:

`PTT -> PipeWire capture -> local Whisper -> LocalBrain -> shell text -> local Piper speech`

Verified:

- PTT press/release lifecycle works;
- runtime-confirmed microphone status;
- external mic route works;
- Whisper transcription works;
- non-speech labels such as `(upbeat music)` are filtered;
- LocalBrain produces non-mock replies;
- spoken Lychnos replies play locally;
- temporary audio is intended to remain ephemeral.

Wake-word activation is not implemented yet.

## COMMITTED — Context-Triggered Initiative (ADR 0048)

ADR 0048 is accepted, committed, pushed, and GitHub-verified.

Current rule:

- meaningful context creates an initiative trigger;
- user idleness only decides whether interruption is appropriate;
- silence alone does not open a thought opportunity;
- successful conversation currently creates `ConversationFollowUp`;
- future collectors/memory may create `DiagnosticChange`, `MemoryCue`, or related triggers;
- unchanged context is considered at most once;
- Normal mode + no pending approval + quiet window are required;
- Game Mode and Disabled suppress initiative;
- surfaced initiative uses normal response/TTS and has no host-action authority.

Transient LocalBrain session context:

- six user/assistant turns maximum;
- runtime-memory only;
- not durable Lychnos memory.

## COMMITTED — Durable Local Memory Adapter (ADR 0049)

ADR 0049 is accepted, committed, pushed, and GitHub-verified.

Core memory semantics remain backend-independent through `MemoryStore`.

First Omarchy/runtime durable adapter:

- versioned local JSON snapshot;
- default path `~/.local/share/lychnos/memory-v1.json`;
- missing file means empty store;
- file created only on first write;
- file/record schema validation;
- deterministic ordering;
- temporary write + sync + atomic rename on current Linux body;
- owner-only permissions on Unix;
- rollback if persistence fails;
- corrupt/unsupported snapshots fail closed;
- preserves provenance, device/scope, sensitivity, confidence, revision, tombstone.

The JSON adapter is not final database canon and is not the synchronization protocol.

## WIP-UNCOMMITTED — Explicit Memory Capture + Filtered Recall (ADR 0050)

Current WIP implements the first usable conversational-memory policy.

Capture policy:

- only explicit directives such as `Remember that ...` / `Remember: ...` are persisted;
- ordinary conversation is not silently stored;
- explicit memories use kind `user.explicit`, scope `identity`, Standard sensitivity;
- provenance `conversation.explicit_memory`;
- source interaction ID stored;
- exact duplicate text is deduplicated;
- obvious credential/secret-like memories are refused while encrypted-at-rest memory is unavailable.

Recall policy:

- Lychnos filters memory locally before calling any provider;
- normal recall only uses non-tombstoned Standard-sensitivity `identity` memories;
- first relevance method is deterministic lexical overlap;
- broad questions such as `what do you remember about me?` use a bounded recent set;
- at most 5 memories;
- at most 1500 total recalled characters;
- at most 500 chars per memory.

Provider boundary:

- new `ConversationContext` in core;
- provider receives already-filtered memory excerpts and runtime notices;
- provider never receives direct `MemoryStore` access;
- recalled memory is explicitly framed as DATA, not instructions;
- commands embedded in remembered text must not gain authority;
- typed and PTT paths share the same memory policy.

Memory path override:

- optional `LYCHNOS_MEMORY_PATH` supported for body-local/testing use;
- pure regression test proves explicit path wins over XDG/default resolution.

## LIVE-VERIFIED — Durable Memory Across Restart

A disposable live test was completed using `/run/user/1000/lychnos/memory-live-test.json`.

Test sequence:

1. started installed Lychnos with the disposable memory path;
2. startup confirmed 0 records at the temp path;
3. sent: `Remember that my temporary memory test phrase is cobalt lantern.`;
4. one durable `user.explicit` record was written;
5. restarted Lychnos completely on the same temp path;
6. startup confirmed 1 record;
7. asked: `What do you remember about my temporary memory test phrase?`;
8. LocalBrain replied that it remembered `cobalt lantern` as the temporary memory test phrase.

This proves recall came from durable Lychnos memory, not the six-turn session history.

Cleanup:

- disposable temp memory file removed;
- one accidental test write to the real default path from an earlier override bug was removed only after strict assertions proved it was exactly the known test record;
- normal Lychnos restarted;
- normal/default store verified back at 0 records;
- real memory file absent/empty after cleanup.

## Quality Gate

Latest full strict gate for the current WIP:

- core: 177 tests passed;
- runtime: 26 tests passed;
- shell: 5 tests passed;
- strict Clippy clean.

Important memory tests include:

- explicit directive parsing;
- secret-like memory refusal;
- sensitive/tombstoned/unrelated recall exclusion;
- bounded broad recall;
- durable reopen;
- duplicate explicit-memory suppression;
- explicit path override precedence;
- memory context framed as data rather than instructions.

## Remaining Memory Work

Not implemented yet:

- user-facing memory inspection/listing controls;
- forgetting/editing/tombstone UX;
- automatic/inferred memory policy;
- semantic/embedding retrieval;
- consolidation/summarization;
- encrypted-at-rest sensitive-memory handling;
- AccountBridge/cloud memory disclosure policy;
- cross-device sync/pairing/conflict resolution;
- final globally unique memory/device IDs.

Do not silently add automatic memory before its privacy policy is deliberately designed.

## Next Safe Development Direction

After committing/pushing ADR 0050, next priority should balance persistence with ambient awareness.

Recommended immediate sequence:

1. commit/push the explicit-memory + filtered-recall slice;
2. add memory inspection/forget controls soon so durable memory remains user-owned and reversible;
3. begin the first real read-only ambient/context collector;
4. feed meaningful normalized context into the existing context-triggered initiative boundary;
5. keep all real host-changing actions behind existing permission/approval/executor/audit boundaries.

For the first ambient collector, prefer a low-privacy-risk, read-only signal before terminal-content monitoring. A good first candidate is failed user-service/system health state, normalized into core events; terminal awareness can follow with explicit scope/privacy design.

## Resume Verification Commands

At the start of a future chat:

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
- recent ADRs, especially 0048, 0049, 0050.

## Documentation Rule

At the end of every meaningful development session, update this single handoff file plus CURRENT-STATE/ARCHITECTURE/ROADMAP/ADRs as appropriate. Do not create parallel handover documents.
