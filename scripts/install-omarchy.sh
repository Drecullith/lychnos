#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$HOME/.local/bin"
LIB_DIR="$HOME/.local/lib/lychnos"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DATA_DIR="$DATA_HOME/lychnos"
APP_DIR="$DATA_HOME/applications"
OMARCHY_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy"
OMARCHY_SHELL_JSON="$OMARCHY_CONFIG_HOME/shell.json"
BAR_DIR="$DATA_DIR/omarchy/bar"
START_AFTER_INSTALL=true

if [[ "${1:-}" == "--no-start" ]]; then
  START_AFTER_INSTALL=false
fi

for command in cargo python3 install; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing required command: $command" >&2
    exit 1
  fi
done

echo "Building Lychnos runtime and shell..."
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml" -p lychnos-runtime
cargo build --release --manifest-path "$ROOT_DIR/prototypes/lychnos-shell/Cargo.toml"

mkdir -p "$BIN_DIR" "$LIB_DIR" "$DATA_DIR/assets" "$BAR_DIR" "$APP_DIR"
install -m 755 "$ROOT_DIR/target/release/lychnos-runtime" "$LIB_DIR/lychnos-runtime"
install -m 755 "$ROOT_DIR/prototypes/lychnos-shell/target/release/lychnos-shell-prototype" "$LIB_DIR/lychnos-shell"
install -m 755 "$ROOT_DIR/integrations/omarchy/bin/lychnos" "$BIN_DIR/lychnos"
install -m 644 "$ROOT_DIR/assets/canon/lychnos-body-v1.png" "$DATA_DIR/assets/lychnos-body-v1.png"
install -m 644 "$ROOT_DIR/integrations/omarchy/bar/LychnosBar.qml" "$BAR_DIR/LychnosBar.qml"
install -m 644 "$ROOT_DIR/integrations/omarchy/bar/lychnos-icon.png" "$BAR_DIR/lychnos-icon.png"

cat >"$APP_DIR/org.lychnos.shell.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Lychnos
Comment=Local-first ambient AI companion
Exec=$BIN_DIR/lychnos start
Icon=$BAR_DIR/lychnos-icon.png
Terminal=false
Categories=Utility;
StartupNotify=false
EOF

if [[ -f "$OMARCHY_SHELL_JSON" ]]; then
  backup="$OMARCHY_SHELL_JSON.before-lychnos-install"
  [[ -f "$backup" ]] || cp "$OMARCHY_SHELL_JSON" "$backup"

  LYCHNOS_BAR_SOURCE="$BAR_DIR/LychnosBar.qml" python3 - "$OMARCHY_SHELL_JSON" <<'PY'
import json
import os
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
layout = data.setdefault("bar", {}).setdefault("layout", {})
center = layout.setdefault("center", [])
entry = {
    "id": "lychnos",
    "type": "qml",
    "source": os.environ["LYCHNOS_BAR_SOURCE"],
}
for index, item in enumerate(center):
    if isinstance(item, dict) and item.get("id") == "lychnos":
        center[index] = entry
        break
else:
    insert_at = len(center)
    for index, item in enumerate(center):
        if isinstance(item, dict) and item.get("id") == "omarchy.system-update":
            insert_at = index + 1
            break
    center.insert(insert_at, entry)

path.write_text(json.dumps(data, indent=2) + "\n")
PY
else
  echo "Warning: Omarchy shell config not found at $OMARCHY_SHELL_JSON" >&2
fi

echo
echo "Lychnos installed."
echo "Launch from a terminal with: lychnos"
echo "Or search for Lychnos in the application launcher."
echo "Useful commands: lychnos stop | lychnos status | lychnos logs"

if $START_AFTER_INSTALL && [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  echo
  "$BIN_DIR/lychnos" start
fi
