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

The current body is procedurally drawn as a temporary visual shell while the canonical body asset pipeline is validated.
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
The prototype uses `gtk4-layer-shell` to float at the top-right of the desktop without reserving screen space or taking keyboard focus.

It currently renders:

- a compact floating black/cyan Lychnos placeholder body;
- a read-only status card;
- distinct Normal, Game Mode, and Disabled expressions/accents;
- a pending-approval notification pip; and
- a sample warning/alert message.

No real system monitoring, provider call, approval action, or host execution is performed.
