# ADR-0024: Runtime Diagnostics Integration

Status: Accepted

## Context

Lychnos already had a diagnostics domain model that was intentionally separate from the mandatory security audit trail.

Until Phase 2, that diagnostics model was not wired into `FoundationRuntime`, so the running prototype produced security-audit records but no ordinary runtime diagnostics.

The project requires these systems to remain separate:

- diagnostics are optional troubleshooting information;
- the security audit is a mandatory safety record.

Disabling ordinary diagnostics must never disable or weaken security auditing.

## Decision

`FoundationRuntime` now owns an independent in-memory diagnostic log alongside its mandatory in-memory security audit log.

The runtime stores:

- an `InMemoryDiagnosticLog`; and
- a `diagnostics_enabled` runtime setting.

`FoundationRuntime::from_config(...)` reads:

```text
config.diagnostics.enabled
```

and controls only ordinary diagnostic emission.

The existing direct `FoundationRuntime::new(...)` constructor keeps diagnostics enabled by default, matching the typed configuration default.

The runtime emits an informational diagnostic when it starts, when diagnostics are enabled.

Action evaluation also emits ordinary diagnostics:

- `Info` when an action would pass the mock execution boundary;
- `Info` when an action is awaiting user approval;
- `Warning` when runtime safety blocks an action.

Diagnostics are exposed read-only through the runtime for the prototype.

## Security Boundary

The security audit remains independent of diagnostics.

The following invariant is explicitly tested:

```text
diagnostics.enabled = false
        |
        v
no diagnostic records

security audit
        |
        v
permission decision still recorded
```

There is no configuration switch that disables the security audit.

Diagnostic failures must not become authorization decisions, and diagnostic records must not be treated as substitutes for security-audit records.

## Consequences

The prototype can now expose useful troubleshooting information without conflating it with the security trail.

Typed configuration now controls a second live subsystem in addition to runtime startup mode.

Future CLI, UI, and development tooling can inspect diagnostics independently from security history.

## Current Limitations

Diagnostics are still in-memory only.

There is no configured diagnostic level threshold yet.

There is no file output, journald integration, rotation, retention policy, structured export, or external sink selection.

Only startup and action-evaluation diagnostics are currently emitted.

The current in-memory diagnostic sink is infallible; behavior for future fallible diagnostic sinks remains to be defined.
