# ADR-0015: Audited Runtime Mode Transitions

Status: Accepted

## Context

Lychnos runtime safety already supports explicit transitions between:

- Normal
- Game Mode
- Disabled

These transitions are security-relevant because they directly control whether Lychnos may perform actions or background work.

The runtime controller itself safely applies those transitions, including failing closed when Game Mode is requested while already Disabled.

However, before this decision, runtime-mode transition requests were not written into the security audit trail.

That left an observability gap: an investigator could see that an action was later blocked by runtime policy without necessarily seeing the mode-change request that established that policy state.

## Decision

The foundation orchestrator exposes audited runtime transition methods:

- `enter_game_mode()`
- `disable()`
- `enable_normal()`

Each requested transition is applied through the existing `RuntimeController` and then appended to the security audit trail as `AuditEventKind::RuntimeModeChanged`.

The audit record includes structured details for:

- requested operation
- previous mode
- resulting mode
- whether the mode actually changed

No-op and blocked transition requests are also audited.

For example, requesting Game Mode while Lychnos is already Disabled records:

```text
from = Disabled
to = Disabled
changed = false
```

This preserves evidence that the request occurred while keeping the fail-closed Disabled state intact.

Foundation tests also prove that:

1. a runtime transition is audited
2. the new runtime state affects the next processing cycle
3. the resulting permission decision is independently audited

In particular, disabling the runtime and then processing a new event produces both:

- a `RuntimeModeChanged` audit record
- a subsequent `PermissionEvaluated` record showing the action was blocked because Lychnos is Disabled

## Consequences

Runtime safety state changes are now part of the append-only security history.

The runtime controller remains the authority for actually changing state; the orchestrator adds audit integration rather than duplicating state-transition logic.

Security analysis can distinguish:

- successful mode changes
- repeated requests for the current mode
- requests that could not weaken Disabled mode
- later action decisions caused by the resulting runtime state

The current audit storage is still in-memory and foundation-phase only. Persistent tamper-resistant audit storage remains a future decision.
