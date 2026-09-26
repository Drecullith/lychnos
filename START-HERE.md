# Project Lychnos — Start Here

This file is the continuity entry point for Project Lychnos.

If you are an AI assistant, contributor, or future development session picking up this repository after a chat/session break, **read this file before changing code**.

## Required Read Order

1. `PROJECT-CANON.md` — stable product truths and non-negotiable architectural rules.
2. `DEVELOPMENT-HANDOFF.md` — exact current branch/WIP/live-machine state and immediate next step.
3. `CURRENT-STATE.md` — implemented capabilities and known gaps.
4. `ARCHITECTURE.md` — module boundaries and current architecture.
5. Relevant recent ADRs under `docs/decisions/`.
6. `SECURITY.md` before changing permissions, execution, monitoring, providers, memory, or system integration.
7. `ROADMAP.md` and `VISION.md` for broader sequencing and product intent.

Then inspect:

```bash
git status --short --branch
git log -15 --oneline
```

Do not assume the working tree is clean.

## Continuity Rule

The repository, not chat memory, is the source of truth.

Do not rely on a previous conversation being available. Any decision necessary to resume development safely must exist in repository documentation or an ADR.

If current code, documentation, and an old chat disagree:

1. do not silently choose one;
2. inspect the most recent ADRs, Git history, and `DEVELOPMENT-HANDOFF.md`;
3. preserve safety/canon;
4. ask before deliberately reopening a locked decision.

## Current Development Philosophy

Project Lychnos is built incrementally:

```text
small boundary
   ↓
tests
   ↓
quality gate
   ↓
document decision
   ↓
commit
   ↓
update handoff
```

Real host authority increases only after the layer beneath it is understandable, tested, auditable, and consistent with `PROJECT-CANON.md`.

## Before Ending a Development Session

Update `DEVELOPMENT-HANDOFF.md` with:

- branch and recent commits;
- dirty/uncommitted files;
- what was completed;
- what is installed/running locally;
- tests/quality-gate result;
- known bugs or design concerns;
- exact next safe step;
- decisions intentionally left open.

If stable canon changed deliberately, update `PROJECT-CANON.md` and create/update an ADR.

See `docs/CONTINUITY-PROTOCOL.md` for the full procedure.
