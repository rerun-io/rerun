#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."

# TODO(emilk): we should probably replace this with https://trunkrs.dev/ or https://github.com/rustwasm/wasm-pack

./scripts/setup_web.sh

OPEN=false
OPTIMIZE=false

while test $# -gt 0; do
  case "$1" in
    -h|--help)
      echo "build_web.sh [--optimize] [--open]"
      echo ""
      echo "  --optimize: Enable optimization step"
      echo "              Runs wasm-opt."
      echo "              NOTE: --optimize also removes debug symbols which are otherwise useful for in-browser profiling."
      echo ""
      echo "  --open:     Open the result in a browser"
      exit 0
      ;;

    -O|--optimize)
      shift
      OPTIMIZE=true
      ;;

    --open)
      shift
      OPEN=true
      ;;

    *)
      break
      ;;
  esac
done

# ./setup_web.sh # <- call this first!

CRATE_NAME="re_viewer"
CRATE_NAME_SNAKE_CASE="${CRATE_NAME//-/_}" # for those who name crates with-kebab-case

BUILD_DIR="web_viewer"

# This is required to enable the web_sys clipboard API which egui_web uses
# https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Clipboard.html
# https://rustwasm.github.io/docs/wasm-bindgen/web-sys/unstable-apis.html
export RUSTFLAGS=--cfg=web_sys_unstable_apis

# Clear output from old stuff:
rm -f ${BUILD_DIR}/${CRATE_NAME_SNAKE_CASE}_bg.wasm

echo "Compiling rust to wasm…"
BUILD=release
cargo build -p ${CRATE_NAME} --release --lib --target wasm32-unknown-unknown

# Get the output directory (in the workspace it is in another location)
TARGET=`cargo metadata --format-version=1 | jq --raw-output .target_directory`

echo "Generating JS bindings for wasm…"
TARGET_NAME="${CRATE_NAME_SNAKE_CASE}.wasm"
wasm-bindgen "${TARGET}/wasm32-unknown-unknown/${BUILD}/${TARGET_NAME}" \
  --out-dir ${BUILD_DIR} --no-modules --no-typescript

if [[ "${OPTIMIZE}" = true ]]; then
  echo "Optimizing wasm…"
  # to get wasm-opt:  apt/brew/dnf install binaryen
  wasm-opt ${BUILD_DIR}/${CRATE_NAME}_bg.wasm -O2 --fast-math -o ${BUILD_DIR}/${CRATE_NAME}_bg.wasm # add -g to get debug symbols
fi

echo "Finished: ${BUILD_DIR}/${CRATE_NAME_SNAKE_CASE}.wasm"

if [ "${OPEN}" = true ]; then
  if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    # Linux, ex: Fedora
    xdg-open http://localhost:9090/index.html
  elif [[ "$OSTYPE" == "msys" ]]; then
    # Windows
    start http://localhost:9090/index.html
  else
    # Darwin/MacOS, or something else
    open http://localhost:9090/index.html
  fi
fi
