#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

export RUSTFLAGS="--deny warnings"

# https://github.com/ericseppanen/cargo-cranky/issues/8
export RUSTDOCFLAGS="--deny warnings --deny rustdoc::missing_crate_level_docs"

cargo check --all-targets --all-features
cargo check -p re_viewer --all-features --target wasm32-unknown-unknown
cargo fmt --all -- --check
cargo cranky --all-targets --all-features -- --deny warnings
cargo test --all-targets --all-features
cargo test --doc --all-features # checks all doc-tests

cargo doc --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

(cd crates/re_log_types && cargo check --no-default-features)
(cd crates/re_viewer && cargo check --no-default-features --features "glow")
(cd crates/re_viewer && cargo check --no-default-features --features "wgpu")
(cd crates/re_web_server && cargo check --no-default-features)
(cd crates/re_ws_comms && cargo check --no-default-features)
(cd crates/rerun && cargo check --no-default-features)
(cd examples/nyud && cargo check --no-default-features)
(cd examples/objectron && cargo check --no-default-features)

(cd crates/re_log_types && cargo check --all-features)
(cd crates/re_viewer && cargo check --all-features)
(cd crates/re_web_server && cargo check --all-features)
(cd crates/re_ws_comms && cargo check --all-features)
(cd crates/rerun && cargo check --all-features)
(cd examples/nyud && cargo check --all-features)
(cd examples/objectron && cargo check --all-features)

./scripts/wasm_bindgen_check.sh

./scripts/lint.py

cargo deny check

echo "All checks passed!"
