use rerun::{
    EncodedImage, InstancePoses3D, Points3D, VideoStream,
    dataframe::EntityPathFilter,
    external::re_log,
    lenses::{LensBuilder, LensesSink, Op},
    sink::GrpcSink,
};

mod conversions;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The path to the MCAP file.
    filepath: std::path::PathBuf,
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
                    Op::func(conversions::list_xyz_struct_to_list_fixed),
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
                    Op::func(conversions::list_xyz_struct_to_list_fixed),
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
                    Op::func(conversions::list_binary_to_list_uint8),
                ],
            )
            .build();

    // TODO(michael): add support for frame_id.
    let video_lens =
        LensBuilder::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
            .add_component_column(
                VideoStream::descriptor_codec(),
                [
                    Op::access_field("format"),
                    Op::func(conversions::list_string_to_list_codec_uint32),
                ],
            )
            .add_component_column(
                VideoStream::descriptor_sample(),
                [
                    Op::access_field("data"),
                    Op::func(conversions::list_binary_to_list_uint8),
                ],
            )
            .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .with_lens(image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens)
        .with_lens(video_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_foxglove_lenses")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
