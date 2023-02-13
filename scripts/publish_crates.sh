#!/usr/bin/env bash
# Publish all our crates
#
# scripts/publish_crates.sh --dry-run
# scripts/publish_crates.sh --execute

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."

DRY_RUN=false
EXECUTE=false

while test $# -gt 0; do
  case "$1" in
    --dry-run)
      shift
      DRY_RUN=true
      ;;

    --execute)
      shift
      EXECUTE=true
      ;;

    *)
      break
      ;;
  esac
done

if [ ${DRY_RUN} = ${EXECUTE} ]; then
    echo "You must pass --dry-run or --execute"
    exit 1
fi

FLAGS=""

if [ ${DRY_RUN} = true ]; then
    FLAGS="--dry-run"
fi

echo $FLAGS


set -x

# IMPORTANT! we need to build an optimized .wasm that will be bundled when we publish re_web_server:
cargo build --release -p re_web_server

# Some of build.rs scripts checks this env-var:
export RERUN_IS_PUBLISHING=yes

echo "Publishing crates…"

cargo publish $FLAGS -p re_log
cargo publish $FLAGS -p re_error
cargo publish $FLAGS -p re_format
cargo publish $FLAGS -p re_string_interner
cargo publish $FLAGS -p re_analytics
cargo publish $FLAGS -p re_memory
cargo publish $FLAGS -p re_tuid
cargo publish $FLAGS -p re_log_types
cargo publish $FLAGS -p re_smart_channel
cargo publish $FLAGS -p re_tensor_ops
cargo publish $FLAGS -p re_ui
cargo publish $FLAGS -p re_arrow_store
cargo publish $FLAGS -p re_data_store
cargo publish $FLAGS -p re_query
cargo publish $FLAGS -p re_sdk_comms
cargo publish $FLAGS -p re_ws_comms
cargo publish $FLAGS -p re_renderer
cargo publish $FLAGS -p re_web_server
cargo publish $FLAGS -p re_viewer
cargo publish $FLAGS -p re_sdk
cargo publish $FLAGS -p rerun
cargo publish $FLAGS -p re_int_histogram

echo "All crates successfully published!"
