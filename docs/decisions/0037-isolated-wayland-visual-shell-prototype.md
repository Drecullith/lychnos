# ADR-0037: Isolated Wayland Visual Shell Prototype

Status: Accepted

## Context

Project Lychnos now has a read-only companion presentation projection suitable for an early desktop body.

The production UI toolkit is intentionally undecided, and platform-independent core CI must not acquire Linux desktop native-library requirements merely to experiment with presentation.

The Omarchy development machine already provides GTK4, gtk4-layer-shell, Wayland, and Hyprland.

## Decision

Create an isolated prototype crate at `prototypes/lychnos-shell`.

The prototype is deliberately **not** a member of the main Cargo workspace. It depends on `lychnos-core` by path and carries its own lockfile so native GUI dependencies stay outside the standard core quality gate.
The prototype uses GTK4 plus gtk4-layer-shell to create a non-exclusive overlay surface on the Omarchy Wayland session.

Current behavior:

- anchors the body near the top-right of the desktop;
- reserves no screen space;
- requests no keyboard interactivity;
- renders a temporary procedural black/cyan Lychnos body;
- renders a compact status card;
- supports safe demo presentation states for Normal, Game Mode, Disabled, pending approval, and alert;
- consumes presentation-domain types only; and
- performs no host monitoring or execution.

The procedural body is temporary. Canonical body-asset loading will be integrated only after the asset-loading path is validated rather than silently replacing the canonical design.
## Security Boundary

The prototype does not contain an executor, approval grant, runtime controller, privileged helper, or direct host-action path.

Its demo states are presentation data, not runtime authority.

Closing, displaying, or changing a demo state cannot authorize an action.

## Consequences

Lychnos can now iterate on its visible desktop presence without pulling Phase 3 system authority or a permanent UI-toolkit decision forward.

The prototype has a separate manual Omarchy build check in addition to the unchanged standard workspace gate.

A future production UI decision may keep, replace, or substantially redesign this prototype without changing the core presentation/security boundary.
