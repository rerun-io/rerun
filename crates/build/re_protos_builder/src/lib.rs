//! This crate contains logic for generating remote store gRPC API types as defined in
//! `re_protos` proto files. We are currently generating both client and server
//! definitions in the same file.
//!

#![expect(clippy::exit)]

/// Generate rust from protobuf definitions. We rely on `tonic_build` to do the heavy lifting.
/// `tonic_build` relies on `prost` which itself relies on `protoc`.
///
/// Note: make sure to invoke this via `pixi run codegen-protos` in order to use the right `protoc` version.
pub fn generate_rust_code<P>(definitions_dir: P, proto_paths: &[P], output_dir: &P)
where
    P: AsRef<std::path::Path>,
{
    let mut prost_config = tonic_prost_build::Config::new();
    prost_config.enable_type_names(); // tonic doesn't expose this option
    prost_config.bytes([
        ".rerun.common.v1alpha1",
        ".rerun.cloud.v1alpha1",
        ".rerun.log_msg.v1alpha1",
        ".rerun.manifest_registry.v1alpha1",
    ]);
    prost_config.enum_attribute(
        ".rerun.cloud.v1alpha1.VectorDistanceMetric",
        "#[derive(serde::Serialize, serde::Deserialize)]",
    );

    if let Err(err) = tonic_prost_build::configure()
        .out_dir(output_dir)
        .build_client(true)
        .build_server(true)
        .build_transport(false) // Small convenience, but doesn't work on web
        .compile_with_config(prost_config, proto_paths, &[definitions_dir])
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
