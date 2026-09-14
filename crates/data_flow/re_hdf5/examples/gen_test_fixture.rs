//! Writes the canonical small HDF5 fixtures used by the Python tests
//! (`rerun_py/tests/assets/hdf5/`). Pure Rust and reproducible:
//!
//! ```sh
//! cargo run -p re_hdf5 --example gen_test_fixture -- <output_dir>
//! ```

use hdf5_pure::{AttrValue, FileBuilder};

fn main() {
    let output_dir = std::env::args()
        .nth(1)
        .expect("usage: gen_test_fixture <output_dir>");
    let output_dir = std::path::Path::new(&output_dir);
    std::fs::create_dir_all(output_dir).expect("failed to create the output directory");

    write_canonical(&output_dir.join("test_data.h5"));
    write_misaligned(&output_dir.join("test_data_misaligned.h5"));
}

/// 5 rows: `/time` (f64 seconds), `/labels` (vlen strings), `/observations`
/// with 2-D `qpos`, 1-D `qvel` and 4-D `images/cam0`, plus a static scalar in
/// `/meta` and attributes on the root, a group, and a dataset.
fn write_canonical(path: &std::path::Path) {
    const NUM_ROWS: usize = 5;

    let mut builder = FileBuilder::new();

    builder.set_attr(
        "description",
        AttrValue::String("canonical re_hdf5 test fixture".into()),
    );
    builder.set_attr("version", AttrValue::I64(1));

    // Float seconds with sub-second steps, so the Python tests can check
    // scale-before-round precision (0.5 s → 500_000_000 ns).
    builder
        .create_dataset("time")
        .with_f64_data(&[0.0, 0.5, 1.0, 1.5, 2.0]);

    builder
        .create_dataset("labels")
        .with_vlen_strings(&["idle", "reach", "grasp", "lift", "place"]);

    let mut observations = builder.create_group("observations");
    observations.set_attr("frequency", AttrValue::F64(30.0));
    observations.set_attr("joints", AttrValue::F64Array(vec![1.0, 2.0, 3.0]));

    #[expect(clippy::cast_precision_loss)]
    let qpos: Vec<f64> = (0..NUM_ROWS * 3).map(|i| i as f64 * 0.1).collect();
    observations
        .create_dataset("qpos")
        .with_f64_data(&qpos)
        .with_shape(&[NUM_ROWS as u64, 3])
        .set_attr("unit", AttrValue::String("rad".into()));

    #[expect(clippy::cast_precision_loss)]
    let qvel: Vec<f32> = (0..NUM_ROWS).map(|i| i as f32).collect();
    observations.create_dataset("qvel").with_f32_data(&qvel);

    // A 4-D uint8 dataset ([N, 2, 2, 3] "images"), deflate-compressed to
    // exercise the decompression path.
    #[expect(clippy::cast_possible_truncation)]
    let cam0: Vec<u8> = (0..NUM_ROWS * 2 * 2 * 3).map(|i| i as u8).collect();
    let mut images = observations.create_group("images");
    images
        .create_dataset("cam0")
        .with_u8_data(&cam0)
        .with_shape(&[NUM_ROWS as u64, 2, 2, 3])
        .with_chunks(&[NUM_ROWS as u64, 2, 2, 3])
        .with_deflate(4);
    observations.add_group(images.finish());
    builder.add_group(observations.finish());

    let mut meta = builder.create_group("meta");
    meta.create_dataset("count")
        .with_i64_data(&[42])
        .with_shape(&[]);
    builder.add_group(meta.finish());

    builder.write(path).expect("failed to write the fixture");
    println!("wrote {}", path.display());
}

/// `/a` has 4 rows, `/b` has 6 — loading without `ignore_datasets` must fail
/// row alignment.
fn write_misaligned(path: &std::path::Path) {
    let mut builder = FileBuilder::new();
    builder
        .create_dataset("a")
        .with_f64_data(&[0.0, 1.0, 2.0, 3.0]);
    builder
        .create_dataset("b")
        .with_f64_data(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    builder.write(path).expect("failed to write the fixture");
    println!("wrote {}", path.display());
}
