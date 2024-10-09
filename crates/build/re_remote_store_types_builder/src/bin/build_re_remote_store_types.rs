#![allow(clippy::unwrap_used)]

use camino::Utf8Path;

const PROTOBUF_DEFINITIONS_DIR_PATH: &str = "crates/store/re_remote_store_types/proto";
const PROTOBUF_REMOTE_STORE_V0_RELATIVE_PATH: &str = "rerun/v0/remote_store.proto";
const RUST_V0_OUTPUT_DIR_PATH: &str = "crates/store/re_remote_store_types/src/v0";

fn main() {
    re_log::setup_logging();

    let workspace_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();

    assert!(
        workspace_dir.join("CODE_OF_CONDUCT.md").exists(),
        "failed to find workspace root"
    );

    let definitions_dir_path = workspace_dir.join(PROTOBUF_DEFINITIONS_DIR_PATH);
    let rust_generated_output_dir_path = workspace_dir.join(RUST_V0_OUTPUT_DIR_PATH);

    re_log::info!("Running codegen for storage node types");

    re_remote_store_types_builder::generate_rust_code(
        definitions_dir_path,
        &[PROTOBUF_REMOTE_STORE_V0_RELATIVE_PATH],
        rust_generated_output_dir_path,
    );
}
