# ADR-0031: Game Mode Running-Work Pause Semantics

Status: Accepted

## Context

Game Mode already blocks new actions and non-essential background work, but ADR-0029 deliberately left already-running work unresolved.

The product requirement is reduced impact during gaming and streaming without weakening the harder Disabled safety boundary.

Treating Game Mode exactly like Disabled would make a temporary performance mode permanently cancel work. Allowing all existing work to continue would undermine Game Mode's purpose.

## Decision

Already-started work receives a reversible Game Mode pause request.

`RuntimeWorkLease::request()` now distinguishes:

- `Continue`;
- `PauseForGameMode`; and
- `CancelForDisabled`.

Entering Game Mode changes a valid existing lease from Continue to PauseForGameMode.

Returning to Normal clears the runtime pause request, but work that already acknowledged the pause does not silently resume.

The simulated executor must explicitly acknowledge the pause and explicitly resume afterward.

Disabled always takes precedence. If Disabled is entered while work is active or paused, the lease becomes permanently cancelled through the disable generation.

## Lifecycle Semantics

```text
Normal -> Running
Game Mode -> PauseRequested
executor acknowledgement -> PausedForGameMode
Normal -> still PausedForGameMode
explicit resume -> Running
Disabled -> CancellationRequested
executor acknowledgement -> StoppedAfterCancellation
```

## Consequences

Game Mode is now a cooperative quiescence boundary rather than a permanent cancellation boundary.

This keeps performance-oriented pausing reversible while preserving Disabled as the stronger irreversible safety request.

A future real executor may need action-specific pause support. Work that cannot truly pause must not falsely report itself as paused.

Automatic Game Mode detection remains outside this decision.
