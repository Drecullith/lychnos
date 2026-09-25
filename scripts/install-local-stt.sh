#!/usr/bin/env bash
set -euo pipefail

CACHE_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/lychnos"
SOURCE_DIR="$CACHE_ROOT/whisper.cpp"
TOOLS_DIR="$CACHE_ROOT/build-tools"
INSTALL_DIR="$HOME/.local/lib/lychnos/stt"
MODEL_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/lychnos/models"
MODEL_NAME="base"
MODEL_FILE="$MODEL_DIR/ggml-base.bin"

for command in python3 git make c++; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Missing required build command: $command" >&2
    exit 1
  fi
done

mkdir -p "$CACHE_ROOT" "$MODEL_DIR"

if [[ ! -x "$TOOLS_DIR/bin/cmake" ]]; then
  echo "Installing Lychnos-local CMake build tool..."
  python3 -m venv "$TOOLS_DIR"
  "$TOOLS_DIR/bin/python" -m ensurepip --upgrade >/dev/null
  "$TOOLS_DIR/bin/python" -m pip install --upgrade pip cmake
fi

export PATH="$TOOLS_DIR/bin:$PATH"

if [[ ! -d "$SOURCE_DIR/.git" ]]; then
  echo "Cloning whisper.cpp..."
  git clone --depth 1 https://github.com/ggml-org/whisper.cpp.git "$SOURCE_DIR"
else
  echo "Updating whisper.cpp..."
  git -C "$SOURCE_DIR" fetch --depth 1 origin master
  git -C "$SOURCE_DIR" reset --hard FETCH_HEAD
fi

echo "Building whisper-cli..."
cmake -S "$SOURCE_DIR" -B "$SOURCE_DIR/build"   -DCMAKE_BUILD_TYPE=Release   -DWHISPER_BUILD_TESTS=OFF   -DWHISPER_BUILD_EXAMPLES=ON
cmake --build "$SOURCE_DIR/build" -j"$(nproc)" --target whisper-cli

if [[ ! -f "$SOURCE_DIR/models/ggml-$MODEL_NAME.bin" ]]; then
  echo "Downloading multilingual Whisper $MODEL_NAME model..."
  (
    cd "$SOURCE_DIR"
    sh ./models/download-ggml-model.sh "$MODEL_NAME"
  )
fi

echo "Installing local STT runtime..."
rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/bin" "$INSTALL_DIR/lib" "$MODEL_DIR"
install -m 755 "$SOURCE_DIR/build/bin/whisper-cli" "$INSTALL_DIR/bin/whisper-cli"

for library in   "$SOURCE_DIR"/build/bin/libwhisper.so*   "$SOURCE_DIR"/build/bin/libggml.so*   "$SOURCE_DIR"/build/bin/libggml-base.so*   "$SOURCE_DIR"/build/bin/libggml-cpu.so*
do
  [[ -e "$library" ]] || continue
  cp -a "$library" "$INSTALL_DIR/lib/"
done

install -m 644 "$SOURCE_DIR/models/ggml-$MODEL_NAME.bin" "$MODEL_FILE"

echo
echo "Local Lychnos STT installed."
echo "Engine: $INSTALL_DIR/bin/whisper-cli"
echo "Model:  $MODEL_FILE"
