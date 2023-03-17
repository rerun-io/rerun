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

cargo check --all-targets --all-features
cargo check -p re_viewer --all-features --target wasm32-unknown-unknown --target-dir target_wasm
cargo fmt --all -- --check
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

./scripts/lint.py

cargo deny --all-features --log-level error --target aarch64-apple-darwin check
cargo deny --all-features --log-level error --target wasm32-unknown-unknown check
cargo deny --all-features --log-level error --target x86_64-pc-windows-msvc check
cargo deny --all-features --log-level error --target x86_64-unknown-linux-musl check

echo "All checks passed!"
