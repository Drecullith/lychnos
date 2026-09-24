# ADR-0002: Normalized Event Envelope

Status: Accepted

## Context

Lychnos will eventually receive events from many sources:

- terminal and shell integrations
- Linux and Omarchy integrations
- applications
- hardware monitors
- runtime controls
- voice and user interaction
- portable Lychnos devices

The core must not understand every platform-specific event format.

## Decision

All external collectors and adapters must translate their input into a platform-independent `Event` before publishing it into the Lychnos core.

The event envelope contains:

- an opaque event identifier
- occurrence timestamp
- source
- event kind
- severity
- sensitivity classification
- optional correlation identifier
- structured payload

Event kinds and sources are represented as opaque names rather than a closed enum so new integrations can add event types without changing the core domain model.

Payloads use a small platform-independent value model during the initial phase.

Generation of globally unique event IDs, serialization format, transport format, and persistence format remain intentionally undecided.

## Consequences

Platform-specific details remain at the adapter boundary.

Core services can process one consistent event shape.

Tests can construct deterministic mock events without access to real Omarchy or hardware.

The event model may evolve through versioned compatibility rules before external integrations are considered stable.
