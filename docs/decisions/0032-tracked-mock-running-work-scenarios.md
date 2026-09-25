# ADR-0032: Tracked Mock Running-Work Scenarios

Status: Accepted

## Context

Runtime leases and `MockRunningWork` could be tested directly, but the Phase 2 scenario runner could not carry one work item across runtime transitions.

That made it difficult to exercise the full pause, resume, disable, and cancellation-acknowledgement story through the same prototype orchestration boundary used by other lifecycle scenarios.

## Decision

`FoundationRuntime` now retains simulation-only running-work records keyed by `ActionId`.

The runtime exposes bounded prototype operations to:

- start mock work in Normal mode;
- inspect its current state;
- acknowledge a Game Mode pause;
- explicitly resume after Game Mode;
- confirm Disabled cancellation; and
- mark work completed.

Duplicate tracked action IDs fail closed, and unknown IDs return an explicit error.

Terminal work remains retained for deterministic inspection during Phase 2 scenarios.

The scenario runner now composes those operations with Normal, Game Mode, and Disabled transitions so one scenario can prove the complete lifecycle.

## Safety Boundary

This registry is simulation state only.

Starting mock work:

- does not execute a command;
- does not write host files;
- does not create a process;
- does not grant or bypass action authorization; and
- does not represent a production executor.

A future real executor must remain behind permission, approval, validation, live runtime checks, and security auditing.

## Consequences

Phase 2 can now exercise already-running work end to end without host integration.

This makes the lifecycle boundary testable before Phase 3 adapters or any real execution authority are introduced.

Real execution tracking, persistence, and execution audit events remain deferred.
