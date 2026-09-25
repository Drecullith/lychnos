# ADR-0021: Pending Action Approval Runtime Flow

Status: Accepted

## Context

ADR-0020 bound approvals to exact action proposals, but approval grants could still be issued directly through the runtime API. Phase 2 needs the runtime itself to own the lifecycle of proposals that are waiting for explicit approval.

Without a pending-action boundary, callers could attempt to approve proposals the runtime was not actually holding, and stale proposals could be confused with newer proposals that reused the same action ID.

## Decision

`FoundationRuntime` now owns an in-memory pending-action store keyed by `ActionId`.

When `evaluate_proposal(...)` returns `AwaitingUserApproval`, the exact proposal is retained by the runtime.

`approve_pending_action(...)` only accepts approval when:

- a proposal with that action ID is currently pending; and
- the caller supplies the exact structured proposal currently stored by the runtime.

If no proposal is pending, the runtime returns `PendingActionError::NotPending`.

If the action ID matches but the proposal has changed, the runtime returns `PendingActionError::ProposalChanged` and issues no approval.

Approval grant issuance is now internal to the runtime and occurs only after the pending proposal has been verified.

After approval is issued, the executor re-evaluates the action against the live runtime state.

If the approved action reaches `WouldExecute`, it is removed from the pending store.

If runtime safety blocks the action, the proposal remains pending.

## Security Properties

This enforces the following behavior:

```text
not pending -> cannot approve

pending proposal A
+
approval request for modified A'
-> rejected before grant issuance

pending proposal A
+
exact approval
+
Normal
-> WouldExecute (mock only) and pending entry removed

pending proposal A
+
exact approval
+
Disabled
-> Blocked and pending entry retained
```

Runtime safety therefore remains authoritative after approval.

The approval and permission evaluations remain separately represented in the security audit log.

## Consequences

The runtime now has an end-to-end prototype approval lifecycle rather than only isolated approval primitives.

Future interaction layers can query pending proposals and present the exact proposal to the user before requesting approval.

A future real executor can build on this flow without exposing direct approval-grant construction.

## Current Limitations

The pending store is in-memory only.

There is no explicit rejection/cancel operation yet.

There is no expiry or timeout for pending actions.

Repeated evaluation of the same action ID replaces the currently stored proposal.

Pending actions are not persisted across process restart.

No real execution is introduced by this decision.
