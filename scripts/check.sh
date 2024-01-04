#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.
# This is likely outdated, but can still be useful.

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."
set -x

export RUSTFLAGS="--deny warnings"

# https://github.com/ericseppanen/cargo-cranky/issues/8
export RUSTDOCFLAGS="--deny warnings --deny rustdoc::missing_crate_level_docs"

export RERUN_WERROR=ON

# Fast things first:
typos
cargo fmt --all -- --check
pixi run lint-rerun
./scripts/ci/cargo_deny.sh
pixi run cpp-test
just py-lint

cargo check --all-targets --all-features
cargo check -p re_viewer --all-features --target wasm32-unknown-unknown --target-dir target_wasm
cargo cranky --all-targets --all-features -- --deny warnings
cargo test --all-targets --all-features
cargo test --doc --all-features # checks all doc-tests

cargo doc --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

(cd crates/re_log_types && cargo check --no-default-features)
(cd crates/re_viewer && cargo check --no-default-features)
(cd crates/re_web_viewer_server && cargo check --no-default-features)
(cd crates/re_ws_comms && cargo check --no-default-features)
(cd crates/rerun && cargo check --no-default-features)
(cd examples/rust/objectron && cargo check --no-default-features)

(cd crates/re_log_types && cargo check --all-features)
(cd crates/re_viewer && cargo check --all-features)
(cd crates/re_web_viewer_server && cargo check --all-features)
(cd crates/re_ws_comms && cargo check --all-features)
(cd crates/rerun && cargo check --all-features)
(cd examples/rust/objectron && cargo check --all-features)

cargo run -p re_build_web_viewer -- --debug

echo "All checks passed!"
