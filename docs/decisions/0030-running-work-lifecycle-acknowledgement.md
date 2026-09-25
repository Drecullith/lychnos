# ADR-0030: Running-Work Lifecycle Acknowledgement

Status: Accepted

## Context

ADR-0029 introduced Disabled-mode cancellation leases for work that had already started, but the first simulation exposed only `Running` and `CancellationRequested`.

That was intentionally incomplete. A cancellation request is not proof that work stopped, and a future real executor must also represent work that finishes normally or cannot honor cancellation.

## Decision

Extend the simulation-only running-work model with executor-owned lifecycle transitions.

Disabled continues to own the cancellation request. The running-work/executor side owns acknowledgement of what actually happened.

The lifecycle distinguishes:

- cancellation requested;
- cancellation unavailable while work remains active;
- normal completion; and
- confirmed stop after cancellation.

A runtime request never silently becomes a confirmed result.

## Completion Race

Work may complete after cancellation has been requested but before cooperative cancellation takes effect.

That race is represented as normal `Completed`, not as a cancelled result.

Once work reaches `Completed` or `StoppedAfterCancellation`, later runtime transitions cannot revive or rewrite that terminal state.

## Security Consequences

Lychnos must report requested state separately from confirmed outcome.

This prevents the UI, audit layer, or a future executor from claiming that:

- a process stopped merely because cancellation was requested;
- prior side effects were rolled back; or
- non-cancellable work was successfully cancelled.

The current implementation remains simulation-only and performs no host-system action.

## Deferred Work

A future real executor still needs:

- concrete cooperative cancellation mechanisms;
- timeout and escalation policy;
- action-specific rollback where possible;
- durable execution-state persistence where justified;
- security-audit events for real execution start, completion, and cancellation acknowledgement.

This ADR does not grant execution authority and does not change the model-to-execution boundary.
