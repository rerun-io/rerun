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

cargo doc --lib --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

(cd comms && cargo check --no-default-features)
(cd log_types && cargo check --no-default-features)
(cd objectron && cargo check --no-default-features)
(cd viewer && cargo check --no-default-features --lib)
(cd web_server && cargo check --no-default-features)

(cd comms && cargo check --all-features)
(cd log_types && cargo check --all-features)
(cd objectron && cargo check --all-features)
(cd viewer && cargo check --all-features)
(cd web_server && cargo check --all-features)

./viewer/wasm_bindgen_check.sh

cargo deny check

echo "All checks passed!"
