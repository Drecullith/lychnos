# ADR-0033: Simulation Work Lifecycle Reporting and Audit

Status: Accepted

## Context

Phase 2 now tracks running-work state across Game Mode and Disabled transitions.

Those states need structured reporting and security-audit visibility, but the prototype must not use real-execution language for work that never touched the host operating system.

The existing `ActionExecutionAttempted` and `ActionExecutionCompleted` audit kinds are therefore not appropriate for simulation-only lifecycle changes.

## Decision

Add stable machine-readable labels to runtime modes and mock running-work states.

`FoundationRuntime` exposes a `MockWorkLifecycleSnapshot` containing:

- action ID;
- current mock running-work state;
- current runtime mode;
- whether the state is terminal; and
- whether executor cooperation is still pending.
Successful mock-work lifecycle transitions append a dedicated `SimulationWorkLifecycleChanged` audit event.

Each record identifies itself explicitly as simulation-only and carries:

- operation;
- previous state;
- resulting state;
- whether the state changed;
- current runtime mode;
- terminal status; and
- cooperation-pending status.

Invalid lifecycle transitions do not append a lifecycle-change record.

## Security Consequences

A simulation audit record must never be interpreted as proof that a host command, process, file write, or privileged operation occurred.

The reserved real-execution audit kinds remain unused until a real restricted executor exists.

This keeps prototype observability useful without overstating authority or effects.
