# ADR 0041: User Runtime and Shell Process Split

## Status

Accepted.

## Context

The floating Wayland shell already renders Lychnos and collects explicit user input, but it must not become the owner of execution authority, runtime safety state, AI-provider calls, approvals, or future system integration.

The installed desktop application also needs a long-running owner for conversation, presentation publication, and approval decisions outside development-only CLI demonstrations.

## Decision

The Omarchy development installation runs two unprivileged user processes:

- `lychnos-runtime` owns `FoundationRuntime`, the canonical persona, conversation-provider calls, presentation publication, and validated approval/rejection consumption;
- `lychnos-shell` owns the floating GTK/Wayland presentation and user-input surface.

The `lychnos` launcher starts, stops, restarts, and reports the status of both processes.

The shell and runtime currently exchange versioned session-local files under `$XDG_RUNTIME_DIR/lychnos/`. This transport is a Phase 2 prototype and may later be replaced by authenticated IPC without changing the authority split.

Neither process has elevated privileges.

## Consequences

- The visual shell remains replaceable and presentation-focused.
- Closing/restarting the shell does not define runtime architecture.
- Approval and future AI/system decisions stay in the runtime owner.
- The same runtime can later service voice, portable, or alternate presentation surfaces.
- Production supervision, crash recovery, authenticated IPC, persistence, and update lifecycle remain future work.
