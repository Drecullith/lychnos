# ADR-0018: Runtime-Aware Collector Suppression

Status: Accepted

## Context

Game Mode and Disabled mode already block actions at the live permission boundary.

The collector pipeline initially still called `Collector::collect()` before that later action decision.

That was safe while collectors were only deterministic mocks, but it would be the wrong boundary for real terminal, journal, service, or hardware collectors. A collector may itself perform non-essential background observation and consume CPU, I/O, power, or other resources before any action proposal exists.

Lychnos requires:

- Game Mode to minimize background impact during gaming and streaming
- Disabled mode to stop ordinary background work immediately
- Disabled to fail closed
- explicit re-enable before ordinary work resumes

## Decision

`FoundationRuntime::collect_once(...)` checks the live runtime controller before polling a collector.

Collector polling is permitted only when:

```text
RuntimeMode::Normal
```

When the runtime is in Game Mode or Disabled:

- `Collector::collect()` is not called
- `collect_once(...)` returns `Ok(None)`
- queued mock events remain untouched
- no foundation processing cycle is created

After an explicit transition back to Normal, collector polling resumes and previously queued mock events can be processed.

The guard uses the existing `RuntimeController::background_work_allowed()` policy rather than duplicating mode checks inside the collector abstraction.

This decision applies to runtime-driven background collector polling. The existing `process_event(...)` boundary remains available for already-normalized events supplied directly to the runtime, and the live permission boundary still protects action evaluation there.

## Consequences

Game Mode now suppresses non-essential collection before observation work begins rather than merely blocking the resulting action later.

Disabled mode now prevents foundation collectors from being polled at all.

Future real collectors inherit the runtime safety boundary without each collector implementing its own mode logic.

Collectors remain independent of `RuntimeController`; orchestration owns the decision about whether background collection may run.

Suppressed collection does not create a separate security audit record. The runtime-mode transition establishing Game Mode or Disabled is already audited, and no collector event or action occurs while polling is suppressed.

Tests prove that:

- Disabled does not poll a queued collector
- Game Mode does not poll a queued collector
- queued events remain pending while collection is suppressed
- explicit re-enable to Normal resumes collection
