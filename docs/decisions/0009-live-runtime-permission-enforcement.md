# ADR-0009: Live Runtime Permission Enforcement

Status: Accepted

## Context

The permission model initially accepted a `RuntimeMode` value supplied by its caller.

That is useful for isolated modeling, but it permits a caller to evaluate an action against a stale runtime-mode snapshot.

For example:

1. a caller reads Normal mode
2. Lychnos is disabled
3. the caller evaluates an action using the previously read Normal value

The permission boundary should consult the current Lychnos runtime safety state directly.

## Decision

The default permission policy evaluates actions against a shared `RuntimeController`.

The policy reads the controller's current mode at evaluation time.

The mode-specific rules remain:

- Normal + read-only action: allowed
- Normal + state-changing or sensitive action: user approval required
- Game Mode: denied
- Disabled: denied

A permission decision is not an execution capability.

Future executors must re-check runtime safety immediately before beginning execution so disabling Lychnos after permission evaluation still fails closed.

No real executor is introduced by this decision.

## Consequences

Permission evaluation is tied to the live Lychnos safety state instead of caller-supplied mode snapshots.

Game Mode and immediate disable become part of the actual authorization path.

A later executor will still require its own final safety check to avoid a time-of-check/time-of-use gap.
