# Lychnos Omarchy bar integration

This custom QML module provides the small Lychnos restore icon used when the
desktop companion shell is minimized.

The Rust shell writes its current presence state to:

`$XDG_STATE_HOME/lychnos/shell-presence`

or, when `XDG_STATE_HOME` is unset:

`~/.local/state/lychnos/shell-presence`

The module is visible while that state is `hidden` or `ghosted`. Clicking the
icon activates the running GTK application's `restore` action, which brings a
minimized orb back or exits click-through Ghost Mode, then changes the state to
`visible`.

This is an Omarchy adapter/integration. It is not part of the platform-
independent Lychnos core.
