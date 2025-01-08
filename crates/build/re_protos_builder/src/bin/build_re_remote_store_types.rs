//! This binary runs the remote store gRPC service codegen manually.
//!
//! It is easiest to call this using `pixi run codegen-protos`,
//! which will set up the necessary tools.

#![allow(clippy::unwrap_used)]

use camino::Utf8Path;

const PROTOS_DIR: &str = "crates/store/re_protos/proto";
const INPUT_V0_DIR: &str = "rerun/v0";
const OUTPUT_V0_RUST_DIR: &str = "crates/store/re_protos/src/v0";

fn main() {
    re_log::setup_logging();

    let workspace_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap();

    // Check for something that only exists in root:
    assert!(
        workspace_dir.join("CODE_OF_CONDUCT.md").exists(),
        "failed to find workspace root"
    );

    let definitions_dir_path = workspace_dir.join(PROTOS_DIR);
    let rust_generated_output_dir_path = workspace_dir.join(OUTPUT_V0_RUST_DIR);
    let proto_paths = std::fs::read_dir(definitions_dir_path.join(INPUT_V0_DIR))
        .unwrap()
        .map(|v| {
            v.unwrap()
                .path()
                .strip_prefix(&definitions_dir_path)
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();

    re_log::info!(
        definitions=?definitions_dir_path,
        output=?rust_generated_output_dir_path,
        protos=?proto_paths,
        "Running codegen for storage node types",
    );

    re_protos_builder::generate_rust_code(
        definitions_dir_path,
        &proto_paths,
        rust_generated_output_dir_path,
    );
}
