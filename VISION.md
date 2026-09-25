# Project Lychnos Vision

## What Lychnos Is

Lychnos is an open-source, local-first ambient AI companion platform.

It is intended to live alongside the user rather than inside a single chat window: observing explicitly permitted system context, explaining what is happening, remembering useful context through Lychnos-owned memory, and proposing actions that remain under user control.

Lychnos is not tied to one AI model, one operating system, or one physical device.

The first integration target is Omarchy, but the core is designed to remain portable.

## The Goal

The long-term goal is a companion that feels persistent without becoming invasive.

A mature Lychnos should be able to:

- notice permitted events from the computer and other Lychnos devices
- explain problems in plain language
- remember durable context independently from any AI provider
- suggest structured actions instead of silently taking control
- ask for approval before meaningful system changes
- remain auditable
- stop immediately when disabled
- reduce itself aggressively during gaming and streaming
- work with cloud AI, local AI, or future providers
- move the same Lychnos identity between desktop and portable bodies

The intended experience is closer to:

> "Terminal 3 hit a snag. I think I know why. Want me to fix it?"

than to an autonomous administrator silently modifying the machine.

## Core Principles

### Local first

Configuration, authoritative memory, event processing, and security records should remain local by default.

Cloud services may be used only when explicitly enabled and should receive only the information required for the enabled request.

No telemetry should be required for the core product.

### User authority

Lychnos may reason, explain, and propose.

The user remains the authority for meaningful actions.

The AI model itself must never become a direct shell-execution authority.

### Model independence

Lychnos identity and memory belong to Lychnos, not to OpenAI, a local model, or any other provider.

Providers are replaceable components.

Changing the reasoning model should not mean losing the companion.

### Platform-independent core

The Rust core should encode Lychnos concepts rather than Omarchy-specific behavior.

Platform collectors and adapters translate native operating-system behavior into the core's normalized boundaries.

Omarchy is the first body of integration, not the definition of Lychnos.

### Safety before autonomy

Runtime safety, permission checks, auditability, and explicit action boundaries are architectural requirements.

They are not optional UI features added later.

### Immediate disable

Disabled mode is intended to be a real runtime invariant.

Ordinary collectors and actions must stop rather than merely hiding the interface.

Re-enabling must be explicit.

### Game-friendly coexistence

Lychnos must be able to coexist with gaming and streaming workloads.

Game Mode is designed to suppress non-essential observation and actions and later will be validated against FPS, latency, thermals, streaming performance, and anti-cheat-sensitive workloads.

### Open development

Lychnos is intended to be developed publicly with understandable architecture, documented decisions, reproducible builds, and a contributor path that does not require hidden infrastructure.

## One Lychnos, Multiple Bodies

Desktop Lychnos and the future pocket device are intended to be two bodies of the same companion.

The portable device is not a separate product identity.

The long-term design is:

```text
              Lychnos Identity
                    |
             Lychnos Memory
                    |
          +---------+---------+
          |                   |
          v                   v
   Desktop / Omarchy     Portable Lychnos
```

The portable form is expected to have its own microphone, speaker, small display, local speech stack, and local model capability for offline use.

Synchronization, encryption, device identity, and conflict resolution remain future design work.

## Canonical Presence

The canonical visual reference is stored at:

`assets/canon/lychnos-body-v1.webp`

The small glowing digital presence, expressive face, and future physical body are different expressions of the same Lychnos identity.

Voice, speech bubbles, and other interaction modes should complement that identity rather than redefine it.

## What Lychnos Is Not

Lychnos is not intended to be:

- a hidden monitoring agent
- a mandatory cloud service
- a root AI daemon
- an unrestricted command runner
- an AI model with permanent machine authority
- an Omarchy-only application
- a replacement for explicit user consent
- a background process that is allowed to degrade gaming or streaming without control

## Current Direction

Development begins with machine-independent foundations:

```text
collectors
   |
normalized events
   |
event bus
   |
analysis
   |
structured action proposals
   |
runtime safety + permission
   |
restricted execution boundary
   |
security audit
```

Real operating-system observation and real execution are deliberately postponed until these boundaries are stable and testable.

## Success

Project Lychnos succeeds if it becomes useful enough to feel present, portable enough to survive model and device changes, and constrained enough that the user can understand and control what it is doing.

The companion should become more capable over time without requiring the user to surrender ownership of the machine, the memory, or the relationship.
