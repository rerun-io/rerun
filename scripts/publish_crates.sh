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

FLAGS="--quiet"

if [ ${DRY_RUN} = true ]; then
    FLAGS="--dry-run"
fi

echo $FLAGS


# IMPORTANT! we need to build an optimized .wasm that will be bundled when we publish `re_web_viewer_server`.
# Normally `re_web_viewer_server/build.rd` builds the wasm/js but during `cargo publish`
# we don't have normal access to the `re_viewer` crate, so the build-script fails,
# (or would have if we didn't set `RERUN_IS_PUBLISHING`).
# So we build the wasm/js pair here that gets bundled in `cargo publish -p re_web_viewer_server` later.
# Between building the wasm and publishing `re_web_viewer_server` there is an opportunity
# to mess things up by running the `re_web_viewer_server`` build-script and over-writing the wasm/js pair.
# This can happen by a bunch of tools like rust-analyzer. We do use different artifact names in debug
# though, so unless you have tools set up to run build scripts with the `--release` flag, we _should_ be fine,
# but just in case:
echo "MAKE SURE RUST ANALYZER, BACON, CARGO-WATCH etc are all OFF!"

set -x

pkillexitstatus=0
sudo pkill -9 rust-analyzer bacon cargo cargo-watch || pkillexitstatus=$?
if [ $pkillexitstatus -eq 0 ]; then
  echo "killed one or more processes"
elif [ $pkillexitstatus -eq 1 ]; then
  echo "no problematic processes found"
elif [ $pkillexitstatus -eq 2 ]; then
  echo "syntax error in the pkill command line"
  exit $pkillexitstatus
elif [ $pkillexitstatus -eq 3 ]; then
  echo "fatal error"
  exit $pkillexitstatus
else
  echo "unexpected error running pkill"
  exit $pkillexitstatus
fi


rm -rf target_wasm # force clean build
rm -f web_viewer/re_viewer_bg.wasm
rm -f web_viewer/re_viewer.js
rm -f web_viewer/re_viewer_debug_bg.wasm
rm -f web_viewer/re_viewer_debug.js
touch crates/re_viewer/src/lib.rs # force recompile of web server
cargo r -p re_build_web_viewer -- --release
cargo r -p re_build_web_viewer -- --debug


# Some of build.rs scripts checks this env-var:
export RERUN_IS_PUBLISHING=yes

echo "Publishing crates…"

cargo publish $FLAGS -p re_build_info
cargo publish $FLAGS -p re_build_build_info
cargo publish $FLAGS -p re_log
cargo publish $FLAGS -p re_int_histogram
cargo publish $FLAGS -p re_error
cargo publish $FLAGS -p re_tuid
cargo publish $FLAGS -p re_format
cargo publish $FLAGS -p re_string_interner
cargo publish $FLAGS -p re_analytics
cargo publish $FLAGS -p re_memory
cargo publish $FLAGS -p re_log_types
cargo publish $FLAGS -p re_smart_channel
cargo publish $FLAGS -p re_log_encoding
cargo publish $FLAGS -p re_tensor_ops
cargo publish $FLAGS -p re_ui
cargo publish $FLAGS -p re_arrow_store
cargo publish $FLAGS -p re_data_store
cargo publish $FLAGS -p re_query
cargo publish $FLAGS -p re_sdk_comms
cargo publish $FLAGS -p re_ws_comms
cargo publish $FLAGS -p re_renderer
cargo publish $FLAGS -p re_build_web_viewer
cargo publish $FLAGS -p re_web_viewer_server
cargo publish $FLAGS -p re_viewer_context
cargo publish $FLAGS -p re_data_ui
cargo publish $FLAGS -p re_viewer
cargo publish $FLAGS -p re_sdk
cargo publish $FLAGS -p rerun
cargo publish $FLAGS -p rerun-cli

echo "All crates successfully published!"
