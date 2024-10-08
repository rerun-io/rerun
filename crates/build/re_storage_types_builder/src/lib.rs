#![allow(clippy::unwrap_used)]

use std::path::Path;

/// Generate rust from from protobuf definitions. We rely on `tonic_build` to do the heavy lifting.
/// `tonic_build` relies on `prost` which itself relies on `protoc`.
///
/// Note: `protoc` that's part of pixi environment will be used.
pub fn generate_rust_code(
    definitions_dir: impl AsRef<Path>,
    proto_paths: &[impl AsRef<Path>],
    output_dir: impl AsRef<Path>,
) {
    tonic_build::configure()
        .out_dir(output_dir.as_ref())
        .build_client(true)
        .build_server(true)
        .build_transport(true)
        .compile_protos(proto_paths, &[definitions_dir])
        .unwrap();
}
