//! Foxglove message lenses for converting protobuf messages to Rerun components.
//!
//! This module provides lenses that transform Foxglove protobuf messages (decoded by
//! `McapProtobufLayer`) into semantic Rerun components.

use arrow::array::{Float32Array, Float64Array, ListArray};
use re_arrow_combinators::cast::PrimitiveCast;
use re_arrow_combinators::map::MapFixedSizeList;
use re_arrow_combinators::map::MapList;
use re_arrow_combinators::reshape::StructToFixedList;
use re_arrow_combinators::Transform as _;
use re_lenses::{Lens, Lenses, Op, OpError, OutputMode};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{Transform3D, VideoStream};

/// Converts a list of structs with `x`, `y`, `z` fields to a list of fixed-size lists with 3 f32 values.
pub fn list_xyz_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Converts a list of structs with `x`, `y`, `z`, `w` fields to a list of fixed-size lists with 4 f32 values (quaternions).
pub fn list_xyzw_struct_to_list_fixed(list_array: &ListArray) -> Result<ListArray, OpError> {
    let pipeline = MapList::new(StructToFixedList::new(["x", "y", "z", "w"]).then(
        MapFixedSizeList::new(PrimitiveCast::<Float64Array, Float32Array>::new()),
    ));
    Ok(pipeline.transform(list_array)?)
}

/// Name of the timestamp field in Foxglove messages and name of the corresponding Rerun timeline.
const FOXGLOVE_TIMESTAMP: &str = "timestamp";

/// Creates a lens for `foxglove.FrameTransforms` messages.
///
/// This lens transforms Foxglove `FrameTransforms` protobuf messages into Rerun `Transform3D` components.
/// Each message can contain multiple transforms, so this uses scatter columns to expand them.
///
/// The Foxglove `FrameTransforms` message structure:
/// ```text
/// message FrameTransforms {
///   repeated FrameTransform transforms = 1;
/// }
///
/// message FrameTransform {
///   google.protobuf.Timestamp timestamp = 1;
///   string parent_frame_id = 2;
///   string child_frame_id = 3;
///   Vector3 translation = 4;
///   Quaternion rotation = 5;
/// }
/// ```
pub fn frame_transforms_lens() -> Lens {
    Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.FrameTransforms:message",
    )
    .output_scatter_columns(|out| {
        out.time(
            FOXGLOVE_TIMESTAMP,
            TimeType::TimestampNs,
            [
                Op::access_field("transforms"),
                Op::flatten(),
                Op::access_field("timestamp"),
                Op::time_spec_to_nanos(),
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
    })
    .expect("failed to build FrameTransforms lens")
    .build()
}

/// Creates a lens for `foxglove.CompressedVideo` messages.
///
/// This lens transforms Foxglove `CompressedVideo` protobuf messages into Rerun `VideoStream` components.
///
/// The Foxglove `CompressedVideo` message structure:
/// ```text
/// message CompressedVideo {
///   google.protobuf.Timestamp timestamp = 1;
///   string frame_id = 2;
///   bytes data = 3;
///   string format = 4;  // e.g., "h264", "h265", "av1"
/// }
/// ```
///
/// Note: We don't set a timestamp timeline for video streams here, to avoid mixing
/// video durations with real time.
pub fn compressed_video_lens() -> Lens {
    Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CompressedVideo:message",
    )
    .output_columns(|out| {
        out.component(
            VideoStream::descriptor_codec(),
            [Op::access_field("format"), Op::string_to_video_codec()],
        )
        .component(
            VideoStream::descriptor_sample(),
            [Op::access_field("data"), Op::binary_to_list_uint8()],
        )
    })
    .expect("failed to build CompressedVideo lens")
    .build()
}

/// Creates a collection of all Foxglove lenses.
///
/// Currently supports:
/// - `foxglove.FrameTransforms` -> `Transform3D`
/// - `foxglove.CompressedVideo` -> `VideoStream`
///
/// More message types can be added in the future.
pub fn foxglove_lenses() -> Lenses {
    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    lenses.add_lens(frame_transforms_lens());
    lenses.add_lens(compressed_video_lens());
    lenses
}
