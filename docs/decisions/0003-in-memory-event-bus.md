# ADR-0003: Initial In-Memory Event Bus

Status: Accepted

## Context

Lychnos components must communicate without becoming tightly coupled to one another.

Collectors and adapters should publish normalized events without knowing which analyzers, audit components, or other services consume them.

The production transport technology is not yet known and must not be chosen based on guesses about future Omarchy integration.

## Decision

The core will begin with a small in-process publish/subscribe event bus implemented using the Rust standard library.

The initial implementation is explicitly a foundation and test implementation, not a permanent transport commitment.

Publishing an event delivers a clone of the normalized `Event` to every currently connected subscriber.

Disconnected subscribers are removed automatically.

The bus reports how many subscribers were attempted, reached, or found disconnected.

Async runtimes, bounded channels, backpressure policy, persistence, cross-process transport, and distributed transport remain intentionally undecided.

## Consequences

Core behavior can be developed and tested without hardware, Omarchy, Tokio, or external messaging dependencies.

Components can begin depending on event publication rather than directly calling each other.

The internal bus implementation can later be replaced or wrapped without changing the normalized event model.
