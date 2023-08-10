//! Log a segmentation image with annotations.
use ndarray::{s, Array, ShapeBuilder};
use rerun::components::{AnnotationContext, Tensor, TensorDataMeaning};
use rerun::datatypes::{AnnotationInfo, ClassDescription, Color, Label};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((200, 300).f());
    data.slice_mut(s![50..150, 50..120]).fill(1);
    data.slice_mut(s![100..180, 130..280]).fill(2);

    let mut image = Tensor::try_from(data.as_standard_layout().view())?;
    image.meaning = TensorDataMeaning::ClassId;

    // create an annotation context to describe the classes
    let annotation = AnnotationContext::from([
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
    MsgSender::new("segmentation")
        .with_component(&[annotation])?
        .send(&rec_stream)?;

    MsgSender::new("segmentation/image")
        .with_component(&[image])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
