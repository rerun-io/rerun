use std::path::PathBuf;

fn main() -> Result<(), std::io::Error> {
    // No need to run this on CI (which means setting up `protoc` etc) since the code is committed
    // anyway.
    if std::env::var("CI").is_ok() {
        return Ok(());
    }

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

    // `include!()` will break LSP & Github navigation, so create an actual source file to make the
    // UX reasonable.
    std::fs::copy(src_path, dst_path).unwrap();

    Ok(())
}
