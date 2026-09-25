# Lychnos Shell Prototype

This is an **Omarchy/Wayland-only visual prototype** for the floating Lychnos desktop body.

It is deliberately isolated from the main Cargo workspace so GTK4 and layer-shell native dependencies do not become requirements for the platform-independent core or standard CI.

## Safety boundary

The shell is presentation-only.

It consumes a versioned `CompanionPresentationEnvelope` containing read-only `CompanionPresentationState`. It does not contain:

- an executor;
- approval grants;
- the runtime controller;
- privileged helpers; or
- direct host action methods.

The canonical segmented black/cyan Lychnos PNG is loaded by the shell. The cyan face is rendered separately so expressions can react to live presentation state.

## Live presentation bridge

The runtime-side publisher writes an authority-free snapshot to:

`$XDG_RUNTIME_DIR/lychnos/presentation-v1.json`

The shell validates the versioned envelope and updates its expression/status from the latest snapshot.

For a safe end-to-end demonstration:

```bash
cargo build --workspace
cargo run -p lychnos-cli -- presentation-demo
```

The demo mutates a real `FoundationRuntime` through simulation-only APIs and publishes these states:

```text
idle -> working -> approval -> Game Mode -> Disabled -> Normal
```

No operating-system action is executed.

## Omarchy dependencies

The development machine currently provides:

- GTK4
- gtk4-layer-shell
- Wayland / Hyprland

Check the prototype with:

```bash
cargo check --manifest-path prototypes/lychnos-shell/Cargo.toml
```

Launch it from the graphical Hyprland session with:

```bash
cargo run --manifest-path prototypes/lychnos-shell/Cargo.toml
```

## Prototype interaction

- drag the Lychnos body to move the overlay;
- the top/right position is remembered in `~/.config/lychnos/shell-position.conf` (or `$XDG_CONFIG_HOME/lychnos/shell-position.conf`);
- double-click the body to hide/show the status card;
- status visibility, Ghost Mode, and position lock persist in `~/.config/lychnos/shell-preferences.conf` (or `$XDG_CONFIG_HOME/lychnos/...`);
- Ghost Mode makes the overlay translucent and click-through using an empty GDK input region;
- minimized/visible/Ghost presence survives shell restarts through the existing state file;
- the Omarchy top-bar icon is the guaranteed recovery path from both minimized and Ghost Mode states;
- right-click for see-through, status, position lock/reset, minimize, and close controls;
- **Minimize to top bar** hides the orb and reveals the Lychnos Omarchy bar icon;
- clicking the small bar icon restores the orb;
- the body has a lightweight idle float/pulse animation.

The Omarchy top-bar adapter lives under `integrations/omarchy/bar/` and is kept outside the platform-independent core.

## Current live mappings

- Normal idle -> happy
- active tracked work -> thinking
- pending approval -> listening/attention + approval indicator
- Game Mode -> focused
- Disabled -> neutral
- warning/error diagnostic -> focused alert treatment

`Excited` and `Speaking` remain in the canonical expression vocabulary for later interaction/voice phases.

This prototype is still not a final UI-toolkit decision.
