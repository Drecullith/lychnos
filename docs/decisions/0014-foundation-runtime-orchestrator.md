# ADR-0014: Foundation Runtime Orchestrator

Status: Accepted

## Context

The Lychnos foundation already has separate modules for:

- normalized events
- the in-memory event bus
- mock analysis
- structured action proposals
- runtime safety
- permission policy
- mock execution
- security auditing
- reusable ID and time providers

The first CLI simulation proved these pieces could work together, but the CLI itself was manually constructing and connecting the pipeline.

That placed orchestration responsibility in an interface layer instead of in the machine-independent core.

## Decision

The core provides a `FoundationRuntime<I, T>` orchestration service parameterized by:

- an `IdProvider`
- a `TimeProvider`

The foundation runtime owns:

- the in-memory event bus
- its internal event subscription
- the runtime safety controller
- the in-memory audit log
- the configured ID provider
- the configured time provider

It exposes two event-entry paths:

- `observe(...)` creates a normalized event using the configured providers and then processes it.
- `process_event(...)` accepts an already-normalized event, allowing future collectors and adapters to feed events into the same runtime boundary.

One processing cycle performs:

1. event publication
2. event receipt
3. mock analysis
4. structured action proposal creation
5. live runtime-safety evaluation
6. permission and mock-execution evaluation
7. security audit append

The result is returned as a `FoundationCycle` containing the publication report, received event, proposal, execution outcome, and current audit-record count.

The CLI now delegates foundation processing to `FoundationRuntime` instead of manually wiring each module.

## Consequences

The CLI becomes a thin client of the core runtime rather than an owner of Lychnos processing logic.

Future interfaces such as desktop UI, voice, tests, collectors, and Omarchy adapters can target the same orchestration boundary.

The current orchestrator intentionally remains foundation-phase code:

- analyzer and executor are still mock implementations
- event transport and audit storage remain in-memory
- orchestration is synchronous
- no operating-system action is executed
- no AI provider is introduced

Future replacement of those implementations must preserve the existing runtime-safety, permission, and audit boundaries.
