use std::path::PathBuf;

fn main() -> Result<(), std::io::Error> {
    prost_build::compile_protos(
        &[
            "dataset/proto/a_r_capture_metadata.proto",
            "dataset/proto/annotation_data.proto",
            "dataset/proto/object.proto",
        ],
        &["dataset/proto"],
    )?;

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let src_path = PathBuf::from(out_dir).join("objectron.proto.rs");
    let dst_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/objectron.rs");

    // TODO: explain why
    std::fs::copy(src_path, dst_path).unwrap();

    Ok(())
}
