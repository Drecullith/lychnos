# ADR-0022: Explicit Pending Action Rejection

Status: Accepted

## Context

ADR-0021 introduced runtime-owned pending actions and exact-proposal approval.

That created a safe positive-consent path, but the negative-consent path also needs explicit semantics. A user saying "no" must remove the exact pending proposal they reviewed without depending on Normal mode or on action-execution permission.

Rejection is distinct from cancellation.

- **Rejection** means the user explicitly refuses a pending proposal.
- **Cancellation** may later represent system-driven or lifecycle-driven removal such as expiry, shutdown, supersession, policy invalidation, or cancellation of work already in progress.

Combining those concepts now would blur user intent and future runtime behavior.

## Decision

`FoundationRuntime::reject_pending_action(...)` rejects one exact pending `ActionProposal`.

The runtime verifies:

- the supplied action ID is currently pending; and
- the supplied proposal structurally matches the exact proposal currently stored under that ID.

If no proposal is pending, the runtime returns `PendingActionError::NotPending`.

If a proposal with the same ID exists but has changed, the runtime returns `PendingActionError::ProposalChanged`.

No `ActionRejected` audit record is produced for either failed rejection attempt.

A successful rejection:

1. removes the pending proposal;
2. appends an `ActionRejected` security-audit record;
3. records the rejecting actor label;
4. records the action ID and source event when available;
5. records action kind, capability, impact, and risk.

Rejection is intentionally **not** gated by runtime mode.

A user must be able to refuse a pending action while Lychnos is in Normal mode, Game Mode, or Disabled.

## Safety Ordering

The pending entry is removed before the audit append.

This preserves user refusal even if a future persistent audit backend fails after the rejection has been accepted.

The current in-memory audit sink is infallible, but the ordering establishes the intended safety property for future fallible persistence.

## Consequences

Lychnos now has explicit positive and negative consent paths for pending actions.

A stale proposal cannot reject a newer replacement that reuses the same action ID.

Disabled mode cannot trap a user in a pending request they are unable to reject.

Future interaction layers can map explicit "Yes" and "No" user actions onto separate runtime APIs.

## Current Limitations

The `rejected_by` value is currently a caller-supplied actor label rather than authenticated user identity.

There is no separate system cancellation API yet.

There is no pending-action expiry, timeout, persistence, or restart recovery.

There is no cancellation of already-running work because no real executor exists yet.
