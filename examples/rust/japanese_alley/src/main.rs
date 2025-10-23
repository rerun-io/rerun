use arrow::array::{Float32Array, Float64Array, ListArray};
use re_arrow_util::transform::{
    BinaryToListUInt8, Cast, MapFixedSizeList, MapList, StructToFixedList, Transform,
};
use rerun::{
    EncodedImage, InstancePoses3D, Points3D,
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

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_japanese_alley")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
