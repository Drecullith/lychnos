# ADR-0036: Read-Only Companion Presentation State

Status: Accepted

## Context

The floating Lychnos body will eventually need to render runtime mode, pending approvals, diagnostics, alerts, and running-work state.

The visual layer must not become an alternate execution path or gain authority merely because it can display security-relevant state.

A direct UI dependency on mutable runtime internals would couple presentation code to approval grants, executors, and safety controllers.

## Decision

Introduce a machine-independent read-only presentation projection.

`CompanionPresentationState` currently contains:

- runtime mode;
- whether ordinary diagnostics are enabled;
- projected pending approvals;
- projected simulation running-work state; and
- the latest ordinary diagnostic record when available.
Pending approval presentation includes only descriptive proposal data required for display.

Tracked-work presentation contains action ID, visible state, terminal status, and whether runtime cooperation is pending.

`FoundationRuntime::presentation_state()` returns owned presentation data rather than references or mutation handles.

Producing the projection does not mutate pending actions, tracked work, audit history, runtime mode, or approval state.

## Security Consequences

The presentation boundary contains no:

- `ApprovalGrant`;
- `RuntimeController`;
- executor;
- mutable runtime reference; or
- direct action method.

A future UI may request actions through explicit application commands, but rendering state alone grants no execution authority.

## Consequences

The desktop visual shell can be developed early against safe simulated/projected state without pulling Phase 3 host authority forward.

The projection can evolve as UI requirements become concrete, while the security-critical runtime remains authoritative.
