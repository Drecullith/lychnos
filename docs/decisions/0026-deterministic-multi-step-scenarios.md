# ADR-0026: Deterministic Multi-Step Scenarios

Status: Accepted

## Context

Phase 2 now has deterministic collectors that can replay events, empty polls, injected collector failures, and recovery.

Those pieces still needed a reusable way to compose multiple operations against the same live foundation runtime so that state, diagnostics, security audit history, and queued collector input could carry forward across a complete scenario.

Large one-off tests inside the orchestrator would make this harder to reuse and harder to evolve.

## Decision

Add a machine-independent `scenario` module to the core.

The module defines ordered `ScenarioStep` operations for:

- polling the supplied collector;
- entering Game Mode;
- disabling Lychnos;
- explicitly returning to Normal mode.

`run_scenario(...)` executes the supplied steps against one existing `FoundationRuntime` and one existing collector.

Each step returns a `ScenarioStepResult`.

Collection failures are captured as step results instead of aborting the entire scenario. This allows later steps to prove deterministic recovery after a failure.

Foundation cycles are boxed inside collection results to keep `ScenarioStepResult` reasonably sized while preserving the complete cycle data.

## Stateful Semantics

A scenario uses the same runtime instance throughout its execution.

That means the following state naturally carries across steps:

- runtime mode;
- security audit history;
- diagnostic history;
- pending runtime state owned by `FoundationRuntime`;
- collector queue position.

The scenario layer does not bypass existing runtime safety or collector suppression rules.

For example, a collector poll attempted while Game Mode or Disabled is active still goes through `FoundationRuntime::collect_once(...)`, so the collector is not consumed until Normal mode is restored.

## Failure Semantics

A collector error is represented as:

```text
ScenarioStepResult::Collection(
    Err(CollectorCycleError::Collector(...))
)
```

The scenario runner records that result and continues to later scenario steps.

It does not swallow or reinterpret the original typed collector error.

## Tested Behaviors

The deterministic scenario layer currently proves:

- ordered processing of multiple events;
- Game Mode pauses collection without consuming queued input;
- explicit return to Normal resumes collection;
- a scenario continues after an injected collector failure;
- a later collector step can recover and process an event;
- Disabled state persists across later scenario steps;
- Game Mode cannot weaken Disabled;
- explicit enable is required to leave Disabled;
- runtime transition audit history accumulates across the scenario.

## Security Boundary

The scenario module is orchestration for simulation only.

It does not:

- execute real operating-system actions;
- grant approval authority;
- bypass live runtime safety;
- bypass the permission boundary;
- alter the meaning of security audit or diagnostics.

All security-sensitive behavior still occurs inside the existing runtime boundaries.

## Consequences

Phase 2 now has a reusable, stateful simulator layer rather than only isolated unit-level runtime calls.

Future deterministic scenarios can combine runtime transitions, event flow, collector failures, approvals, rejections, and later cancellation semantics while preserving one continuous runtime history.

## Current Limitations

The scenario runner currently supports only a small set of runtime-control and collection steps.

It does not yet provide:

- scenario-level approval or rejection steps;
- system-driven cancellation steps;
- timing or delayed steps;
- concurrency;
- multiple collectors;
- analyzer/provider injection;
- persistence/restart simulation;
- resource-budget measurement;
- real Omarchy or Linux adapters.

Those should be added only when the corresponding runtime boundaries are ready.
