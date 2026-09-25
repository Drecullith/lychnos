# ADR-0025: Deterministic Collector Failure Scenarios

Status: Accepted

## Context

Phase 2 needs a simulator that can exercise more than successful event delivery.

Real collectors will eventually encounter transient failures, empty polls, malformed or unavailable sources, and recovery after errors. Those behaviors need to be testable before Lychnos is connected to real Omarchy or Linux observation.

The existing `MockCollector` only modeled FIFO event delivery and an empty queue.

## Decision

Add a machine-independent scripted collector for deterministic simulator scenarios.

`ScriptedCollector` replays an exact ordered sequence of `ScriptedCollectorStep` values:

- `Event(Event)` returns one normalized event;
- `Empty` returns `Ok(None)`;
- `Error(InjectedCollectorError)` returns a deterministic injected collector failure.

The scripted collector may be created from a complete ordered scenario or built incrementally.

Injected collector failures are preserved through the existing typed runtime boundary:

```text
CollectorCycleError::Collector(...)
```

`FoundationRuntime::collect_once(...)` emits an ordinary diagnostic when a collector returns an error and then returns the original typed collector error to the caller.

Collector failures do not create synthetic security-audit records.

After an injected error, later scripted steps remain available, allowing tests to prove that a collector can recover on a later poll and re-enter the normal Lychnos event pipeline.

## Diagnostic and Audit Separation

A collector failure is an operational troubleshooting event, not by itself a security decision.

Therefore:

- ordinary diagnostics record the collector failure;
- the original typed collector error is preserved;
- the mandatory security audit remains untouched unless subsequent processing reaches a security-relevant boundary.

This preserves the distinction established by ADR-0019 and ADR-0024.

## Consequences

The simulator can now model deterministic sequences containing:

```text
event
empty poll
collector failure
recovery event
```

Recovery behavior is reproducible and testable without relying on timing, randomness, filesystem state, or real operating-system services.

This creates a foundation for richer multi-event and failure-injection scenarios later in Phase 2.

## Current Limitations

The scripted collector is test and simulation infrastructure only.

It does not model:

- retries or retry policy;
- exponential backoff;
- time delays;
- concurrency;
- partial event corruption;
- malformed native input;
- event-bus failure;
- analyzer failure;
- persistence failure;
- real Linux or Omarchy collector behavior.

Those behaviors should be introduced only when their owning boundaries exist.
