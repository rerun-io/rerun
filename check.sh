#!/usr/bin/env bash
# This scripts runs various CI-like checks in a convenient way.
set -eux

RUSTFLAGS="-D warnings"
RUSTDOCFLAGS="-D warnings" # https://github.com/emilk/egui/pull/1454

cargo build --all-features
cargo check --all-targets --all-features
cargo check -p re_viewer --all-features --lib --target wasm32-unknown-unknown
cargo fmt --all -- --check
cargo cranky --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc --all-features

cargo doc --lib --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features

(cd crates/re_log_types && cargo check --no-default-features)
(cd crates/re_viewer && cargo check --no-default-features --lib)
(cd crates/re_web_server && cargo check --no-default-features)
(cd crates/re_ws_comms && cargo check --no-default-features)
(cd examples/nyud && cargo check --no-default-features)
(cd examples/objectron && cargo check --no-default-features)

(cd crates/re_log_types && cargo check --all-features)
(cd crates/re_viewer && cargo check --all-features)
(cd crates/re_web_server && cargo check --all-features)
(cd crates/re_ws_comms && cargo check --all-features)
(cd examples/nyud && cargo check --all-features)
(cd examples/objectron && cargo check --all-features)

./crates/re_viewer/wasm_bindgen_check.sh

./scripts/lint.py

cargo deny check

# ----------------
# Python SDK:

python3 -m venv env
source env/bin/activate
python3 -m pip install -r crates/re_sdk_python/requirements.txt
maturin build -m crates/re_sdk_python/Cargo.toml

# -----------------

echo "All checks passed!"
