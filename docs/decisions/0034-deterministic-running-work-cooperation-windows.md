# ADR-0034: Deterministic Running-Work Cooperation Windows

Status: Accepted

## Context

Game Mode pause and Disabled cancellation are cooperative requests.

Phase 2 needs to test delayed acknowledgement, timeout observation, unavailable cancellation, and completion races without selecting an async runtime, scheduler, timer service, or process-control mechanism.

## Decision

Introduce a pure machine-independent cooperation assessment.

The caller supplies:

- the cooperation request being assessed;
- current mock running-work state;
- elapsed milliseconds since the request; and
- a timeout budget in milliseconds.

The assessment returns one of:

- `Waiting`;
- `TimedOut`;
- `Satisfied`;
- `Unavailable`;
- `SupersededByDisabled`;
- `CompletedBeforeCooperation`; or
- `NotRequested`.
The two request classes are:

- Game Mode pause; and
- Disabled cancellation.

A timeout is an observation, not an automatic lifecycle transition.

Therefore a simulated executor may acknowledge a request after the timeout has been observed. Escalation policy remains a separate future decision.

The deterministic scenario runner can now assess cooperation windows and model delayed pause, late cancellation acknowledgement, cancellation-unavailable behavior, and later normal completion.

## Consequences

Tests can express timing-sensitive lifecycle stories without wall-clock sleeps or background tasks.

No production timeout duration is selected by this ADR.

A future real executor must decide how elapsed time is measured, what escalation is permitted, and how any failure to cooperate is surfaced to the user and audit trail.
