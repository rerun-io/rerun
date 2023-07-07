//! Log a segmentation image with annotations.
use ndarray::{s, Array, ShapeBuilder};
use rerun::components::{
    AnnotationContext, AnnotationInfo, ClassDescription, ClassId, ColorRGBA, Label, Tensor,
    TensorDataMeaning,
};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("annotation_context_segmentation").memory()?;

    // create a segmentation image
    let mut data = Array::<u8, _>::zeros((200, 300).f());
    data.slice_mut(s![50..150, 50..120]).fill(1);
    data.slice_mut(s![100..180, 130..280]).fill(2);

    let mut image = Tensor::try_from(data.as_standard_layout().view())?;
    image.meaning = TensorDataMeaning::ClassId;

    // create an annotation context to describe the classes
    let mut annotation = AnnotationContext::default();
    annotation.class_map.insert(
        ClassId(1),
        ClassDescription {
            info: AnnotationInfo {
                id: 1,
                label: Some(Label("red".to_owned())),
                color: Some(ColorRGBA::from_rgb(255, 0, 0)),
            },
            ..Default::default()
        },
    );
    annotation.class_map.insert(
        ClassId(2),
        ClassDescription {
            info: AnnotationInfo {
                id: 2,
                label: Some(Label("green".to_owned())),
                color: Some(ColorRGBA::from_rgb(0, 255, 0)),
            },
            ..Default::default()
        },
    );

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
