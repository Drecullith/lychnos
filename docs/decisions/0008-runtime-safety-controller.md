# ADR-0008: Runtime Safety Controller

Status: Accepted

## Context

Game Mode and immediate disable are core Lychnos safety requirements.

They must not exist only as UI concepts.

Components need a cheap, thread-safe way to determine whether normal background work and actions are currently permitted.

Disabled mode must fail closed: ordinary runtime transitions must not accidentally re-enable Lychnos.

## Decision

The Lychnos core provides a shared `RuntimeController`.

It stores one of three runtime modes:

- Normal
- Game Mode
- Disabled

The controller is thread-safe and uses an atomic state value.

Behavior:

- Normal permits ordinary background work and action evaluation.
- Game Mode blocks actions and non-essential background work.
- Disabled blocks actions and background work.
- `disable()` always transitions to Disabled.
- attempting to enter Game Mode while Disabled does not re-enable Lychnos.
- leaving Disabled requires the explicit `enable_normal()` operation.

The controller reports runtime transitions so future audit integration can record them.

This controller does not claim to interrupt arbitrary operating-system work that has already begun. No real executor exists in this phase.

## Consequences

Runtime safety becomes an enforceable core state rather than only a UI setting.

Future collectors, AI providers, voice services, and executors can consult the same controller.

The Disabled state is resistant to accidental weakening by ordinary mode changes.

Future work will connect runtime transitions to the audit system and cancellation of active tasks.
