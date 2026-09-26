# Project Lychnos — Continuity Protocol

This protocol exists so Project Lychnos can continue safely across ChatGPT chat caps, model changes, long breaks, contributor handoffs, and device changes.

## Source-of-Truth Hierarchy

Use this order when reconstructing context:

1. `PROJECT-CANON.md` — stable locked product/architecture truths.
2. Accepted ADRs under `docs/decisions/` — why specific architectural decisions were made.
3. `DEVELOPMENT-HANDOFF.md` — current operational/WIP state.
4. `CURRENT-STATE.md` — current implemented feature inventory.
5. Current Git working tree and history.
6. `ARCHITECTURE.md`, `SECURITY.md`, `ROADMAP.md`, and `VISION.md`.
7. Old chat history only as supporting context, never as the sole source of a critical project decision.

When two sources conflict, do not silently guess. Prefer newer accepted ADRs and current code/history, while preserving `PROJECT-CANON.md` unless the project deliberately reopens canon.

## Beginning a New Development Chat

Before making code changes:

```bash
cd /home/drec/Work/lychnos
git status --short --branch
git log -15 --oneline
```

Then read:

- `START-HERE.md`
- `PROJECT-CANON.md`
- `DEVELOPMENT-HANDOFF.md`
- relevant recent ADRs
- the section of `CURRENT-STATE.md` related to the next task

If a remote/local development machine is available, also verify the installed runtime rather than assuming it matches the working tree.

## During Development

For each meaningful slice:

1. preserve existing dirty work;
2. make the smallest coherent change;
3. test it;
4. run the appropriate strict quality gate;
5. update architecture/current-state documentation when behavior changed;
6. create or update an ADR for a lasting architectural choice;
7. commit only coherent, reviewed slices.

Do not bundle unrelated experiments merely because they happened in the same chat.

## Before Ending a Development Chat

Update `DEVELOPMENT-HANDOFF.md` with all of the following:

- date;
- current branch;
- ahead/behind state;
- recent milestone commits;
- exact dirty/untracked files;
- what is committed and stable;
- what is experimental/uncommitted;
- what is installed and currently running;
- machine-local configuration relevant to the work;
- latest quality-gate results;
- last successful physical/manual test;
- known bugs and odd behavior;
- design concerns raised but not resolved;
- decisions deliberately deferred;
- immediate next safe step;
- commands needed to verify/resume.

If stable canon changed intentionally:

- update `PROJECT-CANON.md`;
- update/create the appropriate ADR;
- update `CURRENT-STATE.md`, `ARCHITECTURE.md`, or `ROADMAP.md` as needed.

## Handoff Status Labels

Use these labels explicitly:

- **LOCKED** — canon/accepted decision; do not casually reopen.
- **COMMITTED** — implemented and present in Git history.
- **LIVE-VERIFIED** — physically exercised on a real body/device.
- **WIP-UNCOMMITTED** — exists only in the working tree; preserve carefully.
- **PLANNED** — agreed direction, not implemented.
- **OPEN** — deliberately unresolved decision.
- **KNOWN ISSUE** — observed problem that remains.
- **DEFERRED** — intentionally postponed.

These labels stop future sessions from confusing an idea with working code.

## Manual/Physical Test Recording

When a feature depends on real hardware or UI behavior, record the physical test in the handoff.

Examples:

- microphone source and gain;
- PTT press/release behavior;
- actual STT transcript;
- actual TTS playback;
- shell click-through/minimize behavior;
- Game Mode performance;
- wake-word behavior;
- portable-device pairing.

Automated tests do not replace these observations.

## Machine-Local State

Machine-local configuration must be documented as **body-specific**, not core canon.

For the Omarchy development body, current examples include:

- `~/.config/lychnos/brain.env`
- `~/.config/lychnos/voice.env`
- user-local STT/TTS/model runtimes
- installed `lychnos` launcher/runtime/shell

Do not copy those choices into the platform-independent core merely because they work on the development machine.

## Future Chat Bootstrap

A future chat should be able to begin with a request as short as:

> Continue Project Lychnos from the repo handoff.

The assistant should then read the continuity files and inspect Git/runtime state before proposing changes.

## End-of-Session Template

Use this template inside `DEVELOPMENT-HANDOFF.md`:

```text
Date:
Branch:
Git state:
Live runtime:
Last committed milestone:
WIP-uncommitted:
Quality gate:
Last physical test:
Known issues:
Open decisions:
Canon concerns:
Immediate next step:
Resume verification commands:
```

## Important Principle

If something would be painful to rediscover after a chat cap, it belongs in the repository documentation.
