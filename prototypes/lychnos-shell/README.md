# Lychnos Shell Prototype

This is an **Omarchy/Wayland-only visual prototype** for the floating Lychnos desktop body.

It is deliberately isolated from the main Cargo workspace so GTK4 and layer-shell native dependencies do not become requirements for the platform-independent core or standard CI.

## Safety boundary

The shell is presentation-only.

It may consume `CompanionPresentationState`, but it does not contain:

- an executor;
- approval grants;
- the runtime controller;
- privileged helpers; or
- direct host action methods.

The current body is still procedurally drawn while the canonical body asset pipeline is validated, but its armor petals, glossy face disc, cyan expression, hover ring, and alert treatment now intentionally track the locked visual reference much more closely.
## Omarchy dependencies

The development machine currently provides:

- GTK4
- gtk4-layer-shell
- Wayland / Hyprland

Check the prototype with:

```bash
cargo check --manifest-path prototypes/lychnos-shell/Cargo.toml
```

Launch from the graphical Hyprland session with:

```bash
LYCHNOS_DEMO_STATE=normal cargo run --manifest-path prototypes/lychnos-shell/Cargo.toml
```

Supported demo states are `normal`, `game`, `disabled`, `approval`, and `alert`.

## Prototype interaction

- drag the Lychnos body to move the overlay;
- the top/right position is remembered in `~/.config/lychnos/shell-position.conf` (or `$XDG_CONFIG_HOME/lychnos/shell-position.conf`);
- double-click the body to hide/show the status card; and
- the body has a lightweight idle float/pulse animation.

The preference file stores only the two presentation offsets used by this prototype.

The prototype uses `gtk4-layer-shell` to float at the top-right of the desktop without reserving screen space or taking keyboard focus.

It currently renders:

- a compact procedural Lychnos body shaped around the locked segmented-shell / glossy-face / cyan-expression visual language;
- a read-only status card;
- distinct Normal, Game Mode, and Disabled expressions/accents;
- a pending-approval notification pip; and
- a sample warning/alert message;
- drag-to-move ergonomics;
- remembered presentation position; and
- lightweight idle animation.

No real system monitoring, provider call, approval action, or host execution is performed. The only local write performed by the prototype is its own small presentation-position preference file after dragging.
