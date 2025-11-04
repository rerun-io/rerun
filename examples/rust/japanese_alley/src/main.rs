use std::sync::Arc;

use arrow::array::{Array, Float32Array, Float64Array, ListArray};
use re_arrow_util::transform::{
    BinaryToListUInt8, Cast, MapFixedSizeList, MapList, StructToFixedList, Transform,
};
use rerun::{
    EncodedImage, InstancePoses3D, Points3D, VideoStream,
    components::VideoCodec,
    dataframe::EntityPathFilter,
    external::re_log,
    lenses::{Error, LensBuilder, LensesSink, Op},
    sink::GrpcSink,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The path to the MCAP file.
    filepath: std::path::PathBuf,
}

fn list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, Error> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

fn convert_list_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, Error> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(Cast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a ListArray of video format strings to a ListArray of VideoCodec enum values (as UInt32).
// TODO(Michael): can this be done with this fancy pipeline stuff?
fn convert_format_to_codec(input: &ListArray) -> Result<ListArray, Error> {
    // TODO(Michael): move imports out
    use arrow::array::{StringArray, UInt32Array};
    use arrow::datatypes::{DataType, Field};

    let string_array = input
        .values()
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| Error::Other("Expected string array".into()))?;

    // Convert strings to VideoCodec enum values.
    // TODO: this can be done for sure in a better way...
    let mut codec_values = Vec::new();
    for i in 0..input.len() {
        if input.is_null(i) {
            // TODO(Michael): what to do with nulls?
            return Err(Error::Other("Null values are not supported".into()));
        }

        // We should have only one format string per list entry.
        let start = input.value_offsets()[i] as usize;
        let format_str = string_array.value(start);

        // The actual conversion:
        let codec = match format_str.to_lowercase().as_str() {
            "h264" => Ok(VideoCodec::H264),
            "h265" => Ok(VideoCodec::H265),
            _ => Err(Error::Other(
                format!("Unsupported video format: {format_str}").into(),
            )),
        }?;
        codec_values.push(codec as u32);
    }
    let values_array = UInt32Array::from(codec_values);

    let field = Arc::new(Field::new("item", DataType::UInt32, false));
    Ok(ListArray::new(
        field,
        input.offsets().clone(), // TODO(Michael): is this correct?
        std::sync::Arc::new(values_array),
        input.nulls().cloned(),
    ))
}

// TODO: This example is still missing `tf`-style transforms.

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // The following could be improved with columnar archetype APIs.
    let dummy_point = Points3D::new([[0.0f32, 0.0, 0.0]])
        .columns_of_unit_batches()
        .unwrap()
        .next()
        .unwrap();

    // plural
    let instance_poses_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.PosesInFrame:message")
            .add_component_column(
                InstancePoses3D::descriptor_translations(),
                [
                    // Lens operations always work on component-column level.
                    Op::access_field("poses"),
                    Op::flatten(),
                    Op::access_field("position"),
                    Op::func(convert_list_struct_to_list_fixed),
                ],
            )
            .add_static_component_column(
                dummy_point.descriptor.clone(),
                [Op::constant(dummy_point.list_array.clone())],
            )
            .build();

    // singular
    let instance_pose_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.PoseInFrame:message")
            .add_component_column(
                InstancePoses3D::descriptor_translations(),
                [
                    // Lens operations always work on component-column level.
                    Op::access_field("pose"),
                    Op::access_field("position"),
                    Op::func(convert_list_struct_to_list_fixed),
                ],
            )
            .add_static_component_column(
                dummy_point.descriptor,
                [Op::constant(dummy_point.list_array)],
            )
            .build();

    let image_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
            // TODO: We leave out the `format` column because the `png` contents are not a valid MIME type.
            .add_component_column(
                EncodedImage::descriptor_blob(),
                [
                    Op::access_field("data"),
                    Op::func(list_binary_to_list_uint8),
                ],
            )
            .build();

    // Foxglove CompressedVideo has frame_id, format (aka codec) and data (the actual bytes).
    // Lens mapping per input message from Foxglove:
    // frame_id -> ? TODO
    // format -> codec enum (u32)
    // data -> sample
    let video_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
            .add_component_column(
                VideoStream::descriptor_codec(),
                [
                    Op::access_field("format"),
                    Op::func(convert_format_to_codec),
                ],
            )
            .add_component_column(
                VideoStream::descriptor_sample(),
                [
                    Op::access_field("data"),
                    Op::func(list_binary_to_list_uint8),
                ],
            )
            .build();
    // TODO(michael): add frame_id mapping? how?

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens)
        .with_lens(video_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_japanese_alley")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
