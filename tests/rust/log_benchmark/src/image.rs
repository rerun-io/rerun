use rerun::{
    datatypes::TensorBuffer,
    datatypes::{TensorData, TensorDimension},
    external::re_types_core::ArrowBuffer,
};

// About 1gb of image data.
const IMAGE_DIMENSION: u64 = 16_384;
const IMAGE_CHANNELS: u64 = 4;

// How many times we log the image.
// Each time with a single pixel changed.
const NUM_LOG_CALLS: usize = 4;

fn prepare() -> Vec<u8> {
    re_tracing::profile_function!();

    vec![0u8; (IMAGE_DIMENSION * IMAGE_DIMENSION * IMAGE_CHANNELS) as usize]

    // Skip filling with non-zero values, this adds a bit too much extra overhead.
    // image.resize_with(
    //     (IMAGE_DIMENSION * IMAGE_DIMENSION * IMAGE_CHANNELS) as usize,
    //     || {
    //         i += 1;
    //         i as u8
    //     },
    // );
    // image
}

fn execute(mut raw_image_data: Vec<u8>) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let (rec, _storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_benchmark_").memory()?;

    for i in 0..NUM_LOG_CALLS {
        raw_image_data[i] += 1;

        rec.log(
            "test_image",
            // TODO(andreas): We should have a more ergonomic way to create images from raw bytes!
            &rerun::Image::new(TensorData::new(
                vec![
                    TensorDimension::width(IMAGE_DIMENSION),
                    TensorDimension::height(IMAGE_DIMENSION),
                    TensorDimension::depth(IMAGE_CHANNELS),
                ],
                // TODO(andreas): We have to copy the image every time since the tensor buffer wants to
                // take ownership of it.
                // Note that even though our example here is *very* contrived, it's likely that a user
                // will want to keep their image, so this copy is definitely part of our API overhead!
                TensorBuffer::U8(ArrowBuffer::from(raw_image_data.clone())),
            )),
        )?;
    }

    Ok(())
}

/// Log a single large image.
pub fn run() -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare());
    execute(input)
}
