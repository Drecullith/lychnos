# ADR-0013: ID and Time Provider Abstractions

Status: Accepted

## Context

Foundation-phase Lychnos components currently require identifiers and timestamps for events, actions, audit records, memory records, and devices.

Until now, tests and the CLI supplied many of these values directly.

That keeps behavior deterministic, but it also spreads identifier construction and timestamp creation across callers and makes it harder to switch cleanly between:

- deterministic test data
- local simulations
- real wall-clock time
- future globally unique identifiers suitable for multiple devices

The core needs reusable provider boundaries without prematurely committing to a final cross-device identifier format.

## Decision

The core provides two provider traits:

- `IdProvider`
- `TimeProvider`

`IdProvider` can generate typed identifiers for:

- events
- actions
- audit records
- memory records
- devices

The foundation implementation is `SequenceIdProvider`.

It produces deterministic sequential identifiers such as:

```text
event-000001
action-000002
audit-000003
```

This provider is suitable for tests and local simulations only. It is not considered globally unique and is not the final multi-device identifier strategy.

`TimeProvider` exposes Unix epoch milliseconds and typed timestamp helpers for:

- events
- audit records
- memory records

Two foundation implementations exist:

- `FixedTimeProvider` for deterministic tests
- `SystemTimeProvider` for real operating-system wall-clock time

The CLI now uses these provider boundaries instead of hard-coded event and audit identifiers and timestamps.

## Consequences

Identifier and time creation now have explicit replaceable boundaries.

Tests can remain deterministic while runtime code can use real wall-clock time.

Future globally unique identifiers can replace the sequence implementation without requiring each consumer to invent or manage identifiers independently.

The project still deliberately leaves the final portable/multi-device identifier format undecided.
