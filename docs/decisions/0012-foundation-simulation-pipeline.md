# ADR-0012: Foundation Simulation Pipeline

Status: Accepted

## Context

The machine-independent Lychnos foundation now contains:

- normalized events
- an internal event bus
- structured action proposals
- live runtime safety state
- permission policy
- a mock execution boundary
- security audit records

These pieces need to be proven together as one end-to-end Lychnos flow without introducing real operating-system integration or an AI provider.

## Decision

The core provides a deterministic `MockAnalyzer`.

During the foundation phase it converts a normalized event into a read-only structured action proposal.

The mock analyzer exists only to exercise the architecture. It is not intended to represent the eventual AI reasoning layer.

Foundation tests exercise this path:

1. publish normalized event
2. receive event through the internal bus
3. analyze event into a structured action proposal
4. evaluate against live runtime safety state
5. pass through the mock execution boundary
6. append a security audit record

No real system action, shell command, AI request, Omarchy integration, or privileged operation is introduced.

## Consequences

The core can now demonstrate a complete safe Lychnos processing path using only deterministic machine-independent components.

Future analyzers and AI-provider adapters can replace the mock analyzer without bypassing the action, permission, runtime-safety, or audit boundaries.
