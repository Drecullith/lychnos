# ADR-0035: Resource Budget Instrumentation Abstractions

Status: Accepted

## Context

Game Mode is intended to minimize Lychnos impact on gaming, streaming, latency, thermals, and related workloads.

Real measurements belong to later host integration, but Phase 2 needs a portable representation for resource observations and mode-specific budgets before choosing Linux-specific telemetry sources.

## Decision

Add a machine-independent `resource` module.

It defines:

- stable resource metric names;
- explicit units;
- typed resource observations;
- mode-specific resource budgets;
- explicit budget mismatch errors;
- within-budget and exceeded evaluations;
- a generic `ResourceSink`; and
- an in-memory resource observation log for deterministic tests.

Current units are bytes, microseconds, counts, and basis points.
A budget applies only when metric, unit, and runtime mode all match the observation.

Budget evaluation reports exact headroom or excess.

The core does not define final production metric names, measurement intervals, thresholds, sampling frequency, or operating-system data sources yet.

## Consequences

Normal and Game Mode resource expectations can be represented without coupling the core to `/proc`, hardware APIs, a specific telemetry daemon, or an async runtime.

Phase 3 adapters may later supply real observations from the Omarchy machine while preserving the same core budget/evaluation boundary.

This ADR introduces instrumentation structure only. It does not claim that hardware telemetry or gaming/streaming coexistence measurement is implemented.
