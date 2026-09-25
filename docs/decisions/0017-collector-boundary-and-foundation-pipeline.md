# ADR-0017: Collector Boundary and Foundation Pipeline

Status: Accepted

## Context

The Lychnos foundation runtime already accepts normalized events and processes them through the machine-independent safety and audit pipeline.

Before this decision, there was no explicit collector abstraction representing components that observe an external source and produce normalized Lychnos events.

Future integrations will need collectors for sources such as:

- terminal activity
- journals
- service state
- hardware telemetry
- other platform-specific event sources

Those source-specific concerns must not leak into the core runtime.

## Decision

The core defines a generic `Collector` trait.

A collector returns:

- `Ok(Some(Event))` when a normalized event is available
- `Ok(None)` when no event is currently available
- a collector-specific error when collection fails

Collectors produce complete normalized `Event` values before they enter the runtime.

The foundation includes a deterministic FIFO `MockCollector` for tests and simulations.

`FoundationRuntime` exposes `collect_once(...)` to:

1. ask a collector for one normalized event
2. return no cycle when the collector has no event
3. preserve collector errors distinctly
4. pass available events through the existing `process_event(...)` pipeline

The combined path is therefore:

```text
Collector
   |
   v
Normalized Event
   |
   v
Foundation Runtime
   |
   v
Event Bus
   |
   v
Analyzer
   |
   v
Live Runtime Safety
   |
   v
Permission Policy
   |
   v
Mock Execution Boundary
   |
   v
Security Audit
```

Collector failures are represented separately from internal runtime-delivery failures through `CollectorCycleError<E>`.

## Consequences

Future platform-specific collectors can be added without changing the event bus, analyzer, permission policy, executor, or audit boundary.

Collectors are responsible for normalization before handing events to the core.

The core still does not implement real terminal, journal, systemd, hardware, or Omarchy observation.

The foundation now has an end-to-end collector path proving that:

- collector events can flow through the runtime
- an empty collector produces no processing cycle
- collector errors remain distinguishable
- Disabled mode still blocks collector-sourced actions
- collector-sourced permission decisions are audited through the existing security path

The current `MockCollector` is test-only foundation infrastructure and performs no operating-system observation.
