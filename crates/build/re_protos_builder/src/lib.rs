//! This crate contains logic for generating remote store gRPC API types as defined in
//! `re_protos` proto files. We are currently generating both client and server
//! definitions in the same file.
//!

#![allow(clippy::unwrap_used, clippy::exit)]

use std::path::Path;

/// Generate rust from protobuf definitions. We rely on `tonic_build` to do the heavy lifting.
/// `tonic_build` relies on `prost` which itself relies on `protoc`.
///
/// Note: make sure to invoke this via `pixi run codegen-protos` in order to use the right `protoc` version.
pub fn generate_rust_code(
    definitions_dir: impl AsRef<Path>,
    proto_paths: &[impl AsRef<Path>],
    output_dir: impl AsRef<Path>,
) {
    let mut prost_config = prost_build::Config::new();
    prost_config.enable_type_names(); // tonic doesn't expose this option

    if let Err(err) = tonic_build::configure()
        .out_dir(output_dir.as_ref())
        .build_client(true)
        .build_server(true)
        .build_transport(false) // Small convenience, but doesn't work on web
        .compile_protos_with_config(prost_config, proto_paths, &[definitions_dir])
    {
        match err.kind() {
            std::io::ErrorKind::Other => {
                eprintln!("Failed to generate protobuf types:\n{err}");
                std::process::exit(1);
            }
            _ => {
                panic!("{err}");
            }
        }
    }
}
