//! Create and log a segmentation image.
use ndarray::{s, Array, ShapeBuilder};
use rerun::archetypes::AnnotationContext;
use rerun::components::{Tensor, TensorDataMeaning};
use rerun::datatypes::{AnnotationInfo, ClassDescription, Color, Label};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    // create a segmentation image
    let mut image = Array::<u8, _>::zeros((200, 300).f());
    image.slice_mut(s![50..150, 50..120]).fill(1);
    image.slice_mut(s![100..180, 130..280]).fill(2);

    let mut tensor = Tensor::try_from(image.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::ClassId;

    // create an annotation context to describe the classes
    let annotation = AnnotationContext::new([
        ClassDescription {
            info: AnnotationInfo {
                id: 1,
                label: Some(Label("red".into())),
                color: Some(Color::from_rgb(255, 0, 0)),
            },
            ..Default::default()
        },
        ClassDescription {
            info: AnnotationInfo {
                id: 2,
                label: Some(Label("green".into())),
                color: Some(Color::from_rgb(0, 255, 0)),
            },
            ..Default::default()
        },
    ]);

    // log the annotation and the image
    MsgSender::from_archetype("/", &annotation)?.send(&rec_stream)?;

    MsgSender::new("image")
        .with_component(&[tensor])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
