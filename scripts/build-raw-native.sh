#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIRECTORY="${1:-target/raw-native}"
TARGET_ROOT="$ROOT_DIR/target"
OUTPUT_ROOT="$(python3 -c 'import os, sys; print(os.path.abspath(sys.argv[1]))' "$ROOT_DIR/$OUTPUT_DIRECTORY")"
case "$OUTPUT_ROOT" in
  "$TARGET_ROOT"/*) ;;
  *) echo "RAW native output must stay under the workspace target directory" >&2; exit 2 ;;
esac
if [[ -e "$OUTPUT_ROOT" ]]; then
  echo "RAW native output already exists: $OUTPUT_ROOT" >&2
  exit 1
fi

LIBRAW_COMMIT="b93f6e45c194f5df9b02a43b1af9a54b4f41f33f"
CMAKE_COMMIT="eb98e4325aef2ce85d2eb031c2ff18640ca616d3"
SOURCE_ROOT="$OUTPUT_ROOT/source"
LIBRAW_BUILD="$OUTPUT_ROOT/libraw-build"
BRIDGE_BUILD="$OUTPUT_ROOT/bridge-build"
ARTIFACTS="$OUTPUT_ROOT/artifacts"
mkdir -p "$SOURCE_ROOT" "$ARTIFACTS"

clone_pinned() {
  local name="$1"
  local repository="$2"
  local commit="$3"
  local branch="${4:-}"
  local attempt destination actual
  local -a clone_arguments
  for attempt in 1 2 3; do
    destination="$SOURCE_ROOT/$name-$attempt"
    clone_arguments=(clone --depth 1)
    if [[ -n "$branch" ]]; then
      clone_arguments+=(--branch "$branch")
    fi
    if git "${clone_arguments[@]}" "$repository" "$destination" >&2 \
      && git -C "$destination" fetch --depth 1 origin "$commit" >&2 \
      && git -C "$destination" checkout --detach "$commit" >&2; then
      actual="$(git -C "$destination" rev-parse HEAD)"
      if [[ "$actual" = "$commit" ]]; then
        printf '%s\n' "$destination"
        return 0
      fi
    fi
    if [[ "$attempt" -lt 3 ]]; then
      sleep "$((attempt * 2))"
    fi
  done
  echo "$name clone failed after 3 attempts" >&2
  return 1
}

cd "$ROOT_DIR"
LIBRAW_SOURCE="$(clone_pinned LibRaw https://github.com/LibRaw/LibRaw.git "$LIBRAW_COMMIT" 0.22.2)"
CMAKE_SOURCE="$(clone_pinned LibRaw-cmake https://github.com/LibRaw/LibRaw-cmake.git "$CMAKE_COMMIT")"

cmake -S "$CMAKE_SOURCE" -B "$LIBRAW_BUILD" \
  -DLIBRAW_PATH="$LIBRAW_SOURCE" \
  -DBUILD_SHARED_LIBS=ON \
  -DENABLE_EXAMPLES=OFF \
  -DENABLE_OPENMP=OFF \
  -DENABLE_LCMS=OFF \
  -DENABLE_JASPER=OFF \
  -DENABLE_X3FTOOLS=ON
cmake --build "$LIBRAW_BUILD" --config Release --target raw --parallel

if [[ "$(uname -s)" = "Darwin" ]]; then
  RAW_LINK_LIBRARY="$LIBRAW_BUILD/libraw.dylib"
  RAW_BUILT_RUNTIME="$(find "$LIBRAW_BUILD" -type f -name 'libraw.*.dylib' | head -n 1)"
  PACKAGED_RAW="$ARTIFACTS/libraw.25.dylib"
  PACKAGED_BRIDGE="$ARTIFACTS/liblumia_raw_bridge.dylib"
else
  RAW_LINK_LIBRARY="$LIBRAW_BUILD/libraw.so"
  RAW_BUILT_RUNTIME="$(find "$LIBRAW_BUILD" -type f -name 'libraw.so.*' | head -n 1)"
  PACKAGED_RAW="$ARTIFACTS/libraw.so.25"
  PACKAGED_BRIDGE="$ARTIFACTS/liblumia_raw_bridge.so"
fi
test -e "$RAW_LINK_LIBRARY"
test -n "$RAW_BUILT_RUNTIME"

cmake -S plugins/lumia-plugin-raw/native -B "$BRIDGE_BUILD" \
  -DCMAKE_BUILD_TYPE=Release \
  -DLIBRAW_INCLUDE_DIR="$LIBRAW_SOURCE" \
  -DLIBRAW_LIBRARY="$RAW_LINK_LIBRARY"
cmake --build "$BRIDGE_BUILD" --config Release --parallel

if [[ "$(uname -s)" = "Darwin" ]]; then
  BRIDGE_RUNTIME="$BRIDGE_BUILD/liblumia_raw_bridge.dylib"
else
  BRIDGE_RUNTIME="$BRIDGE_BUILD/liblumia_raw_bridge.so"
fi
test -f "$BRIDGE_RUNTIME"
cp -L "$RAW_BUILT_RUNTIME" "$PACKAGED_RAW"
cp -L "$BRIDGE_RUNTIME" "$PACKAGED_BRIDGE"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "bridge=$PACKAGED_BRIDGE" >> "$GITHUB_OUTPUT"
  echo "libraw=$PACKAGED_RAW" >> "$GITHUB_OUTPUT"
  echo "licenses=$LIBRAW_SOURCE" >> "$GITHUB_OUTPUT"
fi
echo "RAW bridge: $PACKAGED_BRIDGE"
echo "LibRaw runtime: $PACKAGED_RAW"
