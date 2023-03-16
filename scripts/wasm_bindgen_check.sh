#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."

if [[ $* == --skip-setup ]]
then
  echo "Skipping setup_web.sh"
else
  echo "Running setup_web.sh"
  scripts/setup_web.sh
fi

CRATE_NAME="re_viewer"

# This is required to enable the web_sys clipboard API which egui_web uses
# https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Clipboard.html
# https://rustwasm.github.io/docs/wasm-bindgen/web-sys/unstable-apis.html
export RUSTFLAGS=--cfg=web_sys_unstable_apis

echo "Building rust…"
BUILD=debug # debug builds are faster
TARGET_DIR="target_wasm"
WASM_TARGET_FILEPATH="${TARGET_DIR}/wasm32-unknown-unknown/${BUILD}/${CRATE_NAME}.wasm"

# make sure we re-build it:
rm -f "${WASM_TARGET_FILEPATH}"

cargo build \
  --package "${CRATE_NAME}" \
  --lib \
  --target wasm32-unknown-unknown \
  --target-dir "${TARGET_DIR}"

echo "Generating JS bindings for wasm…"

# Remove old output (if any):
rm -f "${CRATE_NAME}.js"
rm -f "${CRATE_NAME}_bg.wasm"

wasm-bindgen "${WASM_TARGET_FILEPATH}" --out-dir . --no-modules --no-typescript

# Remove output:
rm -f "${CRATE_NAME}_bg.wasm"
rm -f "${CRATE_NAME}.js"
