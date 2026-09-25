# ADR-0027: System-Driven Pending Action Cancellation

Status: Accepted

## Context

Lychnos already distinguishes two user-facing outcomes for pending actions:

- explicit approval; and
- explicit rejection.

A separate lifecycle case is needed when the runtime determines that a pending request is no longer the live request, even though the user did not reject it.

Treating that case as rejection would misrepresent user intent. Silently replacing or deleting pending state would also weaken the security audit trail.

The first concrete case is supersession: a structurally different proposal reuses an action ID that is already pending.

## Decision

Introduce system-driven cancellation as a distinct pending-action lifecycle operation.

The security audit now includes:

```text
ActionCancelled
```

This is separate from:

```text
ActionRejected
```

The runtime exposes `cancel_pending_action(...)`, which requires the exact currently pending structured proposal and one `PendingActionCancellationReason`.

Current cancellation reasons are:

- `Superseded`
- `PolicyInvalidated`
- `RuntimeLifecycle`

Each reason has a stable audit representation.

System cancellation uses the runtime-owned actor:

```text
foundation-runtime
```

rather than a caller-supplied user actor label.

## Exact-Proposal Requirement

Cancellation requires full structural equality with the currently pending proposal.

An action ID alone is insufficient.

Therefore:

- unknown proposals cannot be cancelled;
- stale proposals cannot cancel newer replacements;
- a system task holding old proposal data cannot invalidate a newer proposal merely because the action ID was reused.

This matches the exact-proposal safety already used by approval and rejection.

## Supersession Semantics

When `evaluate_proposal(...)` receives a structurally different proposal whose action ID is already pending:

1. the existing pending proposal is copied as the proposal being superseded;
2. the old proposal is removed from pending state;
3. an `ActionCancelled` security audit record is appended with reason `superseded`;
4. the new proposal is then evaluated normally;
5. if the new proposal itself requires approval, it becomes the new pending proposal.

The old proposal is therefore not silently overwritten.

## Audit Semantics

Cancellation audit records preserve:

- action ID;
- source event when present;
- action kind;
- required capability;
- impact;
- risk;
- structured cancellation reason.

Cancellation is a security-relevant lifecycle event and therefore belongs in the mandatory security audit, not only ordinary diagnostics.

## Runtime-State Semantics

System cancellation remains available while Lychnos is Disabled.

Disabled mode must not trap stale pending state that the runtime needs to invalidate.

However, entering Disabled does **not** automatically cancel all pending actions in this decision.

A blocked approval attempt while Disabled still leaves the action pending, preserving the previously accepted retry semantics.

Automatic cancellation policies for specific lifecycle transitions should be added only when a concrete rule requires them.

## Rejection Versus Cancellation

The distinction is:

```text
user explicitly refuses proposal
    -> ActionRejected

runtime determines proposal is no longer live
    -> ActionCancelled
```

Cancellation must never be presented as evidence that the user rejected an action.

## Consequences

Pending-action history is now explicit when one proposal supersedes another.

The runtime has a reusable cancellation boundary for later policy and lifecycle behavior without conflating those events with user consent.

The previous same-ID replacement behavior no longer silently overwrites pending state.

## Current Limitations

Cancellation currently applies only to pending actions.

Lychnos still has no real executor and no representation of already-running operating-system work.

This decision does not yet define:

- cancellation of work already executing;
- cooperative cancellation tokens;
- timeout or expiry cancellation;
- restart recovery;
- persistent pending state;
- automatic cancellation when entering Game Mode or Disabled;
- remote-device cancellation.

Those require separate design before real execution exists.
