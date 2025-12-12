use anyhow::{anyhow, bail};
use arrow::array::{
    Array, Float32Array, Float64Array, ListArray, StringArray, StructArray, UInt32Array,
};
use arrow::datatypes::Field;
// `re_arrow_combinators` provides the building blocks from which we compose the conversions.
use re_arrow_combinators::{
    Transform as _,
    cast::{ListToFixedSizeList, PrimitiveCast},
    map::MapFixedSizeList,
    map::MapList,
    reshape::{RowMajorToColumnMajor, StructToFixedList},
    semantic::{BinaryToListUInt8, StringToVideoCodecUInt32, TimeSpecToNanos},
};
use re_log_types::TimeType;
use rerun::external::{re_log, re_sdk_types::reflection::ComponentDescriptorExt};
use rerun::lenses::{Lens, LensesSink, Op, OpError};
use rerun::sink::GrpcSink;
use rerun::{
    ChannelDatatype, ColorModel, ComponentDescriptor, CoordinateFrame, EncodedImage, Image,
    ImageFormat, InstancePoses3D, Pinhole, PixelFormat, Transform3D, TransformAxes3D, VideoStream,
};
use rerun::{dataframe::EntityPathFilter, lenses::OutputMode, Loggable};

/// Foxglove timestamp fields are by definition relative to a custom epoch.
/// In this example, we default to an UNIX epoch timestamp interpretation.
// TODO(michael): consider adding an "auto" option that picks UNIX if timestamps are above a threshold.
#[derive(Clone, Debug, Default, clap::ValueEnum)]
enum Epoch {
    #[default]
    #[clap(name = "unix")]
    /// UNIX epoch (1970-01-01T00:00:00Z)
    Unix,
    #[clap(name = "custom")]
    /// A custom, unknown epoch.
    Custom,
}

impl Epoch {
    /// Rerun `TimeType` for the selected epoch.
    fn time_type(&self) -> TimeType {
        match self {
            Epoch::Unix => TimeType::TimestampNs,
            Epoch::Custom => TimeType::DurationNs,
        }
    }
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// The path to the MCAP file.
    filepath: std::path::PathBuf,

    /// The epoch to use for timestamps.
    /// If set to 'custom', timestamps will be added as a duration since an unknown epoch.
    #[clap(long, default_value = "unix")]
    epoch: Epoch,
}

/// Converts a list of binary arrays to a list of uint8 arrays.
pub fn list_binary_to_list_uint8(input: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(BinaryToListUInt8::<i32>::new()).transform(input)?)
}

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of structs with `x`, `y`, `z`, `w` fields to a list of fixed-size lists with 4 f32 values (quaternions).
pub fn list_xyzw_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    // Arrow transformations can work on any Arrow-level.
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z", "w"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of video codec strings to Rerun `VideoCodec` values (as u32).
pub fn list_string_to_list_codec_uint32(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StringToVideoCodecUInt32::default());
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of structs with i64 `seconds` and i32 `nanos` fields to a list of timestamps in nanoseconds (i64).
pub fn list_timespec_to_list_nanos(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(TimeSpecToNanos::default());
    Ok(pipeline.transform(list_array)?)
}

/// Converts 3x3 row-major f64 matrices stored in variable-size lists to column-major f32 fixed-size lists.
pub fn list_3x3_row_major_to_column_major(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(
        ListToFixedSizeList::new(9)
            .then(RowMajorToColumnMajor::new(3, 3))
            .then(MapFixedSizeList::new(PrimitiveCast::<
                Float64Array,
                Float32Array,
            >::new())),
    );
    Ok(pipeline.transform(list_array)?)
}

/// Converts u32 width and height fields to a `Resolution` component (fixed-size list with two f32 values).
pub fn width_height_to_resolution(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["width", "height"]).then(
        MapFixedSizeList::new(PrimitiveCast::<UInt32Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

fn decode_raw_image_encoding(encoding: &str, dimensions: [u32; 2]) -> anyhow::Result<ImageFormat> {
    let normalized = encoding
        .trim_matches(char::from(0))
        .trim()
        .to_ascii_lowercase();

    match normalized.as_str() {
        "rgb8" => Ok(ImageFormat::rgb8(dimensions)),
        "rgba8" => Ok(ImageFormat::rgba8(dimensions)),
        "rgb16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::RGB,
            ChannelDatatype::U16,
        )),
        "rgba16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::RGBA,
            ChannelDatatype::U16,
        )),
        "bgr8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGR,
            ChannelDatatype::U8,
        )),
        "bgra8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGRA,
            ChannelDatatype::U8,
        )),
        "bgr16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGR,
            ChannelDatatype::U16,
        )),
        "bgra16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::BGRA,
            ChannelDatatype::U16,
        )),
        "mono8" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::L,
            ChannelDatatype::U8,
        )),
        "mono16" => Ok(ImageFormat::from_color_model(
            dimensions,
            ColorModel::L,
            ChannelDatatype::U16,
        )),
        "yuyv" | "yuv422_yuy2" => Ok(ImageFormat::from_pixel_format(
            dimensions,
            PixelFormat::YUY2,
        )),
        "nv12" => Ok(ImageFormat::from_pixel_format(
            dimensions,
            PixelFormat::NV12,
        )),
        // Depth image formats
        "8uc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::U8)),
        "8sc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I8)),
        "16uc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::U16)),
        "16sc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I16)),
        "32sc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::I32)),
        "32fc1" => Ok(ImageFormat::depth(dimensions, ChannelDatatype::F32)),
        format => {
            bail!(
                "Unsupported raw image encoding '{format}'. Supported encodings include: rgb8, rgba8, rgb16, rgba16, bgr8, bgra8, bgr16, bgra16, mono8, mono16, yuyv, yuv422_yuy2, nv12, 8UC1, 8SC1, 16UC1, 16SC1, 32SC1, 32FC1"
            )
        }
    }
}

/// Converts foxglove.RawImage messages into Rerun `ImageFormat` values.
pub fn raw_image_to_image_format(list_array: &ListArray) -> Result<ListArray, OpError> {
    use std::sync::Arc;

    let (_, offsets, values, nulls) = list_array.clone().into_parts();
    let struct_array = values
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| OpError::Other(anyhow!("RawImage data is not a struct array").into()))?;

    let width_array = struct_array
        .column_by_name("width")
        .and_then(|array| array.as_any().downcast_ref::<UInt32Array>())
        .ok_or_else(|| {
            OpError::Other(anyhow!("RawImage message is missing a uint32 'width' field").into())
        })?;
    let height_array = struct_array
        .column_by_name("height")
        .and_then(|array| array.as_any().downcast_ref::<UInt32Array>())
        .ok_or_else(|| {
            OpError::Other(anyhow!("RawImage message is missing a uint32 'height' field").into())
        })?;
    let encoding_array = struct_array
        .column_by_name("encoding")
        .and_then(|array| array.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| {
            OpError::Other(anyhow!("RawImage message is missing an 'encoding' field").into())
        })?;

    let mut formats = Vec::with_capacity(struct_array.len());
    for row_idx in 0..struct_array.len() {
        if struct_array.is_null(row_idx)
            || width_array.is_null(row_idx)
            || height_array.is_null(row_idx)
            || encoding_array.is_null(row_idx)
        {
            formats.push(None);
            continue;
        }

        let width = width_array.value(row_idx);
        let height = height_array.value(row_idx);
        let encoding = encoding_array.value(row_idx);

        let format = decode_raw_image_encoding(encoding, [width, height])
            .map_err(|err| OpError::Other(err.into()))?;
        formats.push(Some(format));
    }

    let format_array =
        ImageFormat::to_arrow_opt(formats).map_err(|err| OpError::Other(err.into()))?;

    Ok(ListArray::new(
        Arc::new(Field::new_list_field(
            format_array.data_type().clone(),
            true,
        )),
        offsets,
        format_array,
        nulls,
    ))
}

// TODO(grtlr): This example is still missing `tf`-style transforms.

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // Name of the timestamp field in Foxglove messages, and name of the corresponding Rerun timeline.
    const TIME_NAME: &str = "timestamp";

    // TODO(grtlr): This can be removed once we have added other 3D primitives.
    // Without this, our viewer heuristics would not spawn a 3D view at all.
    let transform_axes = |length| {
        TransformAxes3D::new(length)
            .columns_of_unit_batches()
            .unwrap()
            .next()
            .unwrap()
    };

    // plural
    let instance_poses_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PosesInFrame:message")
            .output_columns(|out| {
                out.time(
                    TIME_NAME,
                    args.epoch.time_type(),
                    [
                        Op::access_field("timestamp"),
                        Op::func(list_timespec_to_list_nanos),
                    ],
                )
                .component(
                    InstancePoses3D::descriptor_translations(),
                    [
                        // Lens operations always work on component-column level.
                        Op::access_field("poses"),
                        Op::flatten(),
                        Op::access_field("position"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    [
                        Op::access_field("poses"),
                        Op::flatten(),
                        Op::access_field("orientation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
            })?
            .output_static_columns(|out| {
                let axes = transform_axes(0.1);
                out.component(axes.descriptor, [Op::constant(axes.list_array)])
            })?
            .build();

    // singular
    let instance_pose_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.PoseInFrame:message")
            .output_columns(|out| {
                out.time(
                    TIME_NAME,
                    args.epoch.time_type(),
                    [
                        Op::access_field("timestamp"),
                        Op::func(list_timespec_to_list_nanos),
                    ],
                )
                .component(
                    InstancePoses3D::descriptor_translations(),
                    [
                        // Lens operations always work on component-column level.
                        Op::access_field("pose"),
                        Op::access_field("position"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    InstancePoses3D::descriptor_quaternions(),
                    [
                        Op::access_field("pose"),
                        Op::access_field("orientation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
                .component(
                    CoordinateFrame::descriptor_frame(),
                    [Op::access_field("frame_id")],
                )
            })?
            .output_static_columns(|out| {
                let axes = transform_axes(1.0);
                out.component(axes.descriptor, [Op::constant(axes.list_array)])
            })?
            .build();

    let image_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedImage:message")
            .output_columns(|out| {
                out.time(
                    TIME_NAME,
                    args.epoch.time_type(),
                    [
                        Op::access_field("timestamp"),
                        Op::func(list_timespec_to_list_nanos),
                    ],
                )
                // TODO(grtlr): We leave out the `format` column because the `png` contents are not a valid MIME type.
                .component(
                    EncodedImage::descriptor_blob(),
                    [
                        Op::access_field("data"),
                        Op::func(list_binary_to_list_uint8),
                    ],
                )
            })?
            .build();

    let raw_image_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.RawImage:message")
            .output_columns(|out| {
                out.time(
                    TIME_NAME,
                    args.epoch.time_type(),
                    [
                        Op::access_field("timestamp"),
                        Op::func(list_timespec_to_list_nanos),
                    ],
                )
                .component(
                    Image::descriptor_buffer(),
                    [
                        Op::access_field("data"),
                        Op::func(list_binary_to_list_uint8),
                    ],
                )
                .component(
                    Image::descriptor_format(),
                    [Op::func(raw_image_to_image_format)],
                )
            })?
            .build();

    // Note: we don't set a timestamp timeline for video streams here, to avoid mixing video durations with real time.
    // TODO(michael): add support for frame_id.
    let video_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.CompressedVideo:message")
            .output_columns(|out| {
                out.component(
                    VideoStream::descriptor_codec(),
                    [
                        Op::access_field("format"),
                        Op::func(list_string_to_list_codec_uint32),
                    ],
                )
                .component(
                    VideoStream::descriptor_sample(),
                    [
                        Op::access_field("data"),
                        Op::func(list_binary_to_list_uint8),
                    ],
                )
            })?
            .build();

    // TODO(grtlr): This is still work in progress and missing rotation, for example.
    let transforms_lens =
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.FrameTransforms:message")
            .output_scatter_columns_at("transforms", |out| {
                out.time(
                    TIME_NAME,
                    args.epoch.time_type(),
                    [
                        Op::access_field("transforms"),
                        Op::flatten(),
                        Op::access_field("timestamp"),
                        Op::func(list_timespec_to_list_nanos),
                    ],
                )
                .component(
                    Transform3D::descriptor_parent_frame(),
                    [
                        Op::access_field("transforms"),
                        Op::flatten(),
                        Op::access_field("parent_frame_id"),
                    ],
                )
                .component(
                    Transform3D::descriptor_child_frame(),
                    [
                        Op::access_field("transforms"),
                        Op::flatten(),
                        Op::access_field("child_frame_id"),
                    ],
                )
                .component(
                    Transform3D::descriptor_translation(),
                    [
                        Op::access_field("transforms"),
                        Op::flatten(),
                        Op::access_field("translation"),
                        Op::func(list_xyz_struct_to_list_fixed),
                    ],
                )
                .component(
                    Transform3D::descriptor_quaternion(),
                    [
                        Op::access_field("transforms"),
                        Op::flatten(),
                        Op::access_field("rotation"),
                        Op::func(list_xyzw_struct_to_list_fixed),
                    ],
                )
            })?
            .build();

    // Simple pinhole camera calibration lens, setting `image_from_camera` from the `K` matrix.
    // TODO(michael): set child_frame of Pinhole and matching CoordinateFrame for the image to show both in the 3D view.
    let pinhole_lens = Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CameraCalibration:message",
    )
    .output_columns(|out| {
        out.time(
            TIME_NAME,
            args.epoch.time_type(),
            [
                Op::access_field("timestamp"),
                Op::func(list_timespec_to_list_nanos),
            ],
        )
        .component(
            Pinhole::descriptor_resolution(),
            [Op::func(width_height_to_resolution)],
        )
        .component(
            Pinhole::descriptor_image_from_camera(),
            [
                Op::access_field("K"),
                Op::func(list_3x3_row_major_to_column_major),
            ],
        )
        .component(
            Pinhole::descriptor_parent_frame(),
            [Op::access_field("frame_id")],
        )
    })?
    .build();

    // Destructures a custom GripperStatus message into component columns.
    const GRIPPER_STATUS_SCHEMA_NAME: &str = "schemas.proto.GripperStatus";
    let gripper_status_lens = Lens::for_input_column(
        EntityPathFilter::all(),
        format!("{GRIPPER_STATUS_SCHEMA_NAME}:message"),
    )
    .output_columns(|out| {
        out.time(
            TIME_NAME,
            args.epoch.time_type(),
            [
                Op::access_field("timestamp"),
                Op::func(list_timespec_to_list_nanos),
            ],
        )
        .component(
            ComponentDescriptor::partial("claw_state")
                .with_builtin_archetype(GRIPPER_STATUS_SCHEMA_NAME),
            [Op::access_field("claw_state")],
        )
        .component(
            ComponentDescriptor::partial("error_code")
                .with_builtin_archetype(GRIPPER_STATUS_SCHEMA_NAME),
            [Op::access_field("error_code")],
        )
        .component(
            ComponentDescriptor::partial("position")
                .with_builtin_archetype(GRIPPER_STATUS_SCHEMA_NAME),
            [Op::access_field("position")],
        )
        .component(
            ComponentDescriptor::partial("current")
                .with_builtin_archetype(GRIPPER_STATUS_SCHEMA_NAME),
            [Op::access_field("current")],
        )
    })?
    .build();

    // Destructures a custom JointState message into component columns.
    const JOINT_STATE_SCHEMA_NAME: &str = "schemas.proto.JointState";
    let joint_states_lens = Lens::for_input_column(
        EntityPathFilter::all(),
        format!("{JOINT_STATE_SCHEMA_NAME}:message"),
    )
    .output_columns(|out| {
        out.time(
            TIME_NAME,
            args.epoch.time_type(),
            [
                Op::access_field("timestamp"),
                Op::func(list_timespec_to_list_nanos),
            ],
        )
        .component(
            ComponentDescriptor::partial("joint_names")
                .with_builtin_archetype(JOINT_STATE_SCHEMA_NAME),
            [Op::access_field("joint_names")],
        )
        .component(
            ComponentDescriptor::partial("joint_positions")
                .with_builtin_archetype(JOINT_STATE_SCHEMA_NAME),
            [Op::access_field("joint_positions")],
        )
        .component(
            ComponentDescriptor::partial("joint_velocities")
                .with_builtin_archetype(JOINT_STATE_SCHEMA_NAME),
            [Op::access_field("joint_velocities")],
        )
        .component(
            ComponentDescriptor::partial("joint_efforts")
                .with_builtin_archetype(JOINT_STATE_SCHEMA_NAME),
            [Op::access_field("joint_efforts")],
        )
    })?
    .build();

    let lenses_sink = LensesSink::new(GrpcSink::default())
        .output_mode(OutputMode::ForwardUnmatched)
        .with_lens(image_lens)
        .with_lens(raw_image_lens)
        .with_lens(instance_pose_lens)
        .with_lens(instance_poses_lens)
        .with_lens(video_lens)
        .with_lens(transforms_lens)
        .with_lens(pinhole_lens)
        .with_lens(gripper_status_lens)
        .with_lens(joint_states_lens);

    let (rec, _serve_guard) = args.rerun.init("rerun_example_mcap_protobuf")?;
    rec.set_sink(Box::new(lenses_sink));
    rec.log_file_from_path(args.filepath, None, false)?;

    Ok(())
}
