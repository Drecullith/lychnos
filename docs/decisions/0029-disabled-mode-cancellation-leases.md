# ADR-0029: Disabled-Mode Cancellation Leases for Running Work

Status: Accepted

## Context

Lychnos already blocks new actions when the runtime is in Game Mode or Disabled, and pending actions have explicit approval, rejection, and system-cancellation semantics.

That still left one important safety gap: work that had already started before Lychnos entered Disabled had no machine-independent way to observe that shutdown request.

A future real executor must not treat "Disabled" as merely a gate for new work. Already-started work also needs a cancellation signal, while avoiding false promises that cancellation automatically rolls back side effects or instantly terminates arbitrary operating-system activity.

## Decision

Introduce a machine-independent cancellation lease for work that starts while the runtime is in Normal mode.

`RuntimeController::try_begin_work(...)` now issues a `RuntimeWorkLease` only when the runtime is Normal.

The runtime state carries both:

- the current `RuntimeMode`; and
- a disable generation.

Entering Disabled advances the disable generation.

A lease records the generation that was current when work began. It reports cancellation requested when either:

- the runtime is currently Disabled; or
- the runtime's disable generation no longer matches the lease generation.

Therefore, once a Disabled transition invalidates a lease, later returning to Normal does not revive that old work.

## Lifecycle Semantics

The accepted behavior is:

```text
Normal
  -> work begins
  -> lease issued
  -> Disabled
  -> disable generation advances
  -> old lease reports cancellation requested
  -> explicit re-enable to Normal
  -> old lease remains cancelled
  -> newly started work receives a fresh lease
```

New work cannot begin while the runtime is in Game Mode or Disabled.

## Simulation Boundary

Phase 2 adds `MockRunningWork` and `MockRunningWorkState` so this lifecycle can be tested without introducing real operating-system execution.

The mock running-work representation has only two observable states:

- `Running`
- `CancellationRequested`

It performs no shell commands, file writes, process control, or other host-system effects.

## Security Consequences

Disabled now has a machine-independent cancellation signal for already-started work in addition to blocking new work.

A Disabled transition is latched into existing leases through the generation change. Re-enabling Normal operation cannot silently resurrect work that existed before the disable boundary.

This is a cancellation request, not a rollback guarantee.

A future real executor must still:

- check runtime safety immediately before actual work begins;
- cooperate with cancellation requests while work is running;
- distinguish cancellation requested from cancellation confirmed;
- report completion or cancellation outcomes accurately;
- avoid claiming that already-applied side effects were undone unless an action-specific rollback actually occurred.

Pending-action cancellation remains a separate lifecycle from running-work cancellation.

## Game Mode

Game Mode continues to block new actions and non-essential background work.

This ADR deliberately does not define whether already-running work should be cancelled, paused, drained, or allowed to complete when Game Mode is entered.

That policy remains a separate decision because gaming/streaming coexistence requirements may differ from the hard Disabled safety boundary.

## Current Limitations

This boundary does not yet provide:

- a real executor;
- task, process, or thread cancellation;
- cancellation acknowledgement;
- cancellation timeout handling;
- completion state for running work;
- action-specific rollback;
- persistence across restart;
- automatic cancellation auditing for real work;
- Game Mode semantics for already-running work.

Those remain later Phase 2 boundaries before real execution is introduced.
