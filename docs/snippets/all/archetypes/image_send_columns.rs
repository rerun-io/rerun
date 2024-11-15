use ndarray::{s, Array, ShapeBuilder};
use rerun::Archetype as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_send_columns").spawn()?;

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
    rec.log_static("images", &[&format as _, &rerun::Image::indicator() as _])?;

    // Split up the image data into several components referencing the underlying data.
    let image_size_in_bytes = width * height * 3;
    let blob = rerun::datatypes::Blob::from(images.into_raw_vec_and_offset().0);
    let image_column = times
        .iter()
        .map(|&t| {
            let byte_offset = image_size_in_bytes * (t as usize);
            rerun::components::ImageBuffer::from(
                blob.clone() // Clone is only a reference count increase, not a full copy.
                    .sliced(byte_offset..(byte_offset + image_size_in_bytes)),
            )
        })
        .collect::<Vec<_>>();

    // Send all images at once.
    let timeline_values = rerun::TimeColumn::new_sequence("step", times);
    rec.send_columns("images", [timeline_values], [&image_column as _])?;

    Ok(())
}
