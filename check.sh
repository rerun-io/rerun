#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.
set -eux

RUSTDOCFLAGS="-D warnings" # https://github.com/emilk/egui/pull/1454

cargo build --all-features
cargo check --workspace --all-targets --all-features
cargo check -p viewer --all-features --lib --target wasm32-unknown-unknown
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --  -D warnings -W clippy::all
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features

cargo check -p viewer --no-default-features

cargo doc --lib --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

./viewer/wasm_bindgen_check.sh

cargo deny check

echo "All checks passed!"
