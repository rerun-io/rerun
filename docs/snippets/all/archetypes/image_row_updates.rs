//! Update an image over time.
//!
//! See also the `image_column_updates` example, which achieves the same thing in a single operation.

use ndarray::{Array, ShapeBuilder as _, s};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_row_updates").spawn()?;

    for t in 0..20 {
        rec.set_time_sequence("time", t);

        let mut image = Array::<u8, _>::zeros((200, 300, 3).f());
        image.slice_mut(s![.., .., 2]).fill(255);
        image
            .slice_mut(s![50..150, (t * 10)..(t * 10 + 100), 1])
            .fill(255);

        rec.log(
            "image",
            &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image)?,
        )?;
    }

    Ok(())
}
