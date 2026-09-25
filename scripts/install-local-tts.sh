#!/usr/bin/env bash
set -euo pipefail

VENV="$HOME/.local/lib/lychnos/tts-venv"
VOICE_ID="en_GB-alan-medium"
VOICE_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/lychnos/voices/$VOICE_ID"
MODEL="$VOICE_DIR/$VOICE_ID.onnx"
CONFIG="$VOICE_DIR/$VOICE_ID.onnx.json"
BASE_URL="https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_GB/alan/medium"

for command in python3 curl; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing required command: $command" >&2
    exit 1
  fi
done

echo "Installing Lychnos-local Piper TTS..."
python3 -m venv "$VENV"
"$VENV/bin/python" -m ensurepip --upgrade >/dev/null
"$VENV/bin/python" -m pip install --upgrade pip
"$VENV/bin/python" -m pip install "piper-tts==1.8.0"

mkdir -p "$VOICE_DIR"

if [[ ! -f "$MODEL" ]]; then
  echo "Downloading Lychnos voice model..."
  curl -L --fail --retry 3     -o "$MODEL"     "$BASE_URL/$VOICE_ID.onnx?download=true"
fi

if [[ ! -f "$CONFIG" ]]; then
  echo "Downloading Lychnos voice config..."
  curl -L --fail --retry 3     -o "$CONFIG"     "$BASE_URL/$VOICE_ID.onnx.json?download=true"
fi

echo
echo "Local Lychnos TTS installed."
echo "Engine: $VENV/bin/piper"
echo "Voice:  $MODEL"
