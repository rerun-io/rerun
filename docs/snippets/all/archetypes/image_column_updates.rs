//! Update an image over time, in a single operation.
//!
//! This is semantically equivalent to the `image_row_updates` example, albeit much faster.

use ndarray::{Array, ShapeBuilder as _, s};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_column_updates").spawn()?;

    // Timeline on which the images are distributed.
    let times = (0..20).collect::<Vec<i64>>();

    // Create a batch of images with a moving rectangle.
    let width = 300;
    let height = 200;
    let mut images = Array::<u8, _>::zeros((times.len(), height, width, 3).f())
        .as_standard_layout() // Make sure the data is laid out as we expect it.
        .into_owned();
    images.slice_mut(s![.., .., .., 2]).fill(255);
    for &t in &times {
        let t = t as usize;
        images
            .slice_mut(s![t, 50..150, (t * 10)..(t * 10 + 100), 1])
            .fill(255);
    }

    // Log the ImageFormat and indicator once, as static.
    let format = rerun::components::ImageFormat::rgb8([width as _, height as _]);
    rec.log_static("images", &rerun::Image::update_fields().with_format(format))?;

    // Split up the image data into several components referencing the underlying data.
    let image_size_in_bytes = width * height * 3;
    let timeline_values = rerun::TimeColumn::new_sequence("step", times.clone());
    let buffer = images.into_raw_vec_and_offset().0;
    rec.send_columns(
        "images",
        [timeline_values],
        rerun::Image::update_fields()
            .with_many_buffer(buffer.chunks(image_size_in_bytes))
            .columns_of_unit_batches()?,
    )?;

    Ok(())
}
