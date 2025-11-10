use arrow::array::{Float32Array, Float64Array, ListArray};

use re_log_types::TimeType;
use rerun::{
    EncodedImage, InstancePoses3D, Points3D, VideoStream,
    dataframe::EntityPathFilter,
    external::re_log,
    lenses::{Error, LensBuilder, LensesSink, Op},
    sink::GrpcSink,
};

// `re_arrow_combinators` provides the building blocks from which we compose the conversions.
use re_arrow_combinators::{
    Transform as _,
    cast::PrimitiveCast,
    map::MapFixedSizeList,
    map::MapList,
    reshape::StructToFixedList,
    semantic::{BinaryToListUInt8, StringToVideoCodecUInt32, TimeSpecToNanos},
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The path to the MCAP file.
    filepath: std::path::PathBuf,
}

/// Converts a list of binary arrays to a list of uint8 arrays.
pub fn list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, Error> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, Error> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of video codec strings to Rerun `VideoCodec` values (as u32).
pub fn list_string_to_list_codec_uint32(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(StringToVideoCodecUInt32::default());
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of structs with i64 `seconds` and i32 `nanos` fields to a list of timestamps in nanoseconds (i64).
pub fn list_timespec_to_list_nanos(list_array: &ListArray) -> Result<ListArray, Error> {
    let pipeline = MapList::new(TimeSpecToNanos::default());
    Ok(pipeline.transform(list_array)?)
}

// TODO(grtlr): This example is still missing `tf`-style transforms.

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
                    Op::func(list_xyz_struct_to_list_fixed),
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
                    Op::func(list_xyz_struct_to_list_fixed),
                ],
            )
            .add_static_component_column(
                dummy_point.descriptor,
                [Op::constant(dummy_point.list_array)],
            )
            .build();

    let image_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
            // TODO(grtlr): We leave out the `format` column because the `png` contents are not a valid MIME type.
            .add_component_column(
                EncodedImage::descriptor_blob(),
                [
                    Op::access_field("data"),
                    Op::func(list_binary_to_list_uint8),
                ],
            )
            .build();

    // TODO(michael): add support for frame_id.
    let video_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
            .add_time_column(
                "timestamp",
                TimeType::TimestampNs,
                [
                    Op::access_field("timestamp"),
                    Op::func(list_timespec_to_list_nanos),
                ],
            )
            .add_component_column(
                VideoStream::descriptor_codec(),
                [
                    Op::access_field("format"),
                    Op::func(list_string_to_list_codec_uint32),
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

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens)
        .with_lens(video_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_mcap_protobuf")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
