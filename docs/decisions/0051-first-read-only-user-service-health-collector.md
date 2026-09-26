# ADR 0051: First Read-Only Ambient User-Service Health Collector

## Status

Accepted.

## Context

Lychnos must become ambient and context-aware rather than remaining only a conversational orb.

The first real host collector should prove the platform-adapter -> normalized event -> foundation runtime -> bounded initiative path without beginning with high-privacy-risk sources such as terminal contents, shell history, journals, browser data, or keystrokes.

User-service failure state is a useful first signal: it is read-only, structured, low-volume, and can indicate that something in the user's session is genuinely wrong.

## Decision

The first real Omarchy ambient collector observes failed systemd user services.

The body-specific runtime adapter:

- invokes `systemctl --user --failed --no-legend --plain --no-pager` read-only;
- polls at most once every 15 seconds;
- parses and validates only failed user-service unit names;
- stores the last failed-unit set in runtime memory;
- emits no event when a healthy/unchanged baseline remains unchanged;
- emits one current-condition event if Lychnos starts while failed user services already exist;
- emits a change event only when the failed-unit set changes;
- emits an informational recovery event when all previously failed user services recover;
- does not read terminal contents, process arguments, journal messages, file contents, browser data, or keystrokes.

Normalized event source is `omarchy.systemd.user`.

Initial/current failure and changed-failure events are Error severity. Full recovery is Info severity. Event sensitivity is currently Standard because only validated user-unit names and counts are collected.

Payload is bounded structured data containing failed count, current failed unit names, newly failed units, and recovered units.

The collector is wired through the existing `Collector` / `FoundationRuntime::collect_once` boundary. Therefore Game Mode and Disabled suppression remains owned by the core runtime: when background work is not allowed, the collector is not polled.

The current foundation analyzer/executor remain simulation-only. Processing this real observation through the foundation pipeline does not execute a host action.

## Initiative observation context

`InitiativeContext` now carries bounded `InitiativeObservation` values.

A normalized ambient event may create a `DiagnosticChange` trigger carrying a short Lychnos-owned summary of the observation.

Normalized observation text is explicitly framed to the intelligence provider as contextual DATA rather than instructions.

A real ambient observation may open an initiative consideration window even when there has been no prior conversation session. Silence alone still cannot do so.

The existing quiet-time, runtime-mode, pending-approval, cooldown, and initiative-policy checks remain authoritative before any proactive response may surface.

## Consequences

- Lychnos now has its first real read-only ambient host signal.
- Ambient awareness enters through the same normalized event boundary designed during the foundation phase.
- The first collector has a deliberately narrow privacy footprint.
- Repeated unchanged state does not create repeated model calls or chatter.
- Game Mode and Disabled continue to suppress collection through the core runtime boundary.
- Initiative gains real observed context without giving the model direct operating-system access.
- Terminal awareness, journal context, hardware telemetry, and broader service/process observation remain separate future adapters with their own privacy/scoping decisions.
