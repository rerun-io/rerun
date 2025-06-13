#![allow(clippy::unwrap_used)]

const IMAGE_DIMENSION: u64 = 1024;
const IMAGE_CHANNELS: u64 = 4;

// How many times we log the image.
// Each time with a single pixel changed.
const NUM_LOG_CALLS: usize = 20_000;

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

fn execute(rec: &mut rerun::RecordingStream, mut raw_image_data: Vec<u8>) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    for i in 0..NUM_LOG_CALLS {
        raw_image_data[i] += 1; // Change a single pixel of the image data, just to make sure we transmit something different each time.

        let image = {
            re_tracing::profile_scope!("rerun::Image::from_rgba32");
            rerun::Image::from_rgba32(
                // TODO(andreas): We have to copy the image every time since the tensor buffer wants to
                // take ownership of it.
                // Note that even though our example here is *very* contrived, it's likely that a user
                // will want to keep their image, so this copy is definitely part of our API overhead!
                raw_image_data.clone(),
                [IMAGE_DIMENSION as _, IMAGE_DIMENSION as _],
            )
        };

        re_tracing::profile_scope!("log");
        rec.log("test_image", &image)?;
    }

    Ok(())
}

/// Log a single large image.
pub fn run(rec: &mut rerun::RecordingStream) -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare());
    execute(rec, input)
}
