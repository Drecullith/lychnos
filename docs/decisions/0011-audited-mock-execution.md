# ADR-0011: Audited Mock Execution

Status: Accepted

## Context

Lychnos now has:

- structured actions
- live runtime safety state
- permission evaluation
- an audit model
- a mock execution boundary

These components should not remain isolated.

Security-relevant authorization results must be recordable as part of the same flow that evaluates an action.

## Decision

The `MockExecutor` gains an audited evaluation path.

The audited path:

1. evaluates the structured action against the live `RuntimeController`
2. produces the normal mock execution outcome
3. creates a `PermissionEvaluated` audit record
4. associates the audit record with the action and originating event when available
5. records the resulting authorization decision
6. appends the record through the generic `AuditSink`

Audit identifiers and timestamps are supplied by the caller during this phase.

This avoids prematurely selecting:

- a UUID implementation
- a system clock abstraction
- persistent audit storage

Audit-sink failures are propagated to the caller.

The mock executor still performs no real system operation.

## Consequences

Permission and mock execution behavior can now be tested together with the security audit trail.

Future orchestration code can rely on a single audited boundary.

Real executors must preserve this audit relationship while also re-checking runtime safety immediately before actual execution.
