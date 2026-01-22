//! Foxglove message lenses for converting protobuf messages to Rerun components.
//!
//! This module provides lenses that transform Foxglove protobuf messages (decoded by
//! `McapProtobufLayer`) into semantic Rerun components.

use arrow::array::{Float32Array, Float64Array, ListArray, UInt32Array};
use re_arrow_combinators::cast::{ListToFixedSizeList, PrimitiveCast};
use re_arrow_combinators::map::MapFixedSizeList;
use re_arrow_combinators::map::MapList;
use re_arrow_combinators::reshape::{RowMajorToColumnMajor, StructToFixedList};
use re_arrow_combinators::Transform as _;
use re_lenses::{Lens, Lenses, Op, OpError, OutputMode};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::{
    CoordinateFrame, EncodedImage, InstancePoses3D, Pinhole, Transform3D, TransformAxes3D,
    VideoStream,
};

// =============================================================================
// Helper functions for Arrow transformations
// =============================================================================

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

/// Helper to create static `TransformAxes3D` for visualization.
fn transform_axes(length: f32) -> re_sdk_types::SerializedComponentColumn {
    TransformAxes3D::new(length)
        .columns_of_unit_batches()
        .expect("failed to serialize TransformAxes3D")
        .next()
        .expect("TransformAxes3D should have at least one column")
}

// =============================================================================
// Constants
// =============================================================================

/// Name of the timestamp field in Foxglove messages and name of the corresponding Rerun timeline.
const FOXGLOVE_TIMESTAMP: &str = "timestamp";

// =============================================================================
// Lens definitions
// =============================================================================

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

/// Creates a lens for `foxglove.CompressedImage` messages.
///
/// This lens transforms Foxglove `CompressedImage` protobuf messages into Rerun `EncodedImage` components.
///
/// The Foxglove `CompressedImage` message structure:
/// ```text
/// message CompressedImage {
///   google.protobuf.Timestamp timestamp = 1;
///   string frame_id = 2;
///   bytes data = 3;
///   string format = 4;  // e.g., "jpeg", "png", "webp"
/// }
/// ```
pub fn compressed_image_lens() -> Lens {
    Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CompressedImage:message",
    )
    .output_columns(|out| {
        out.time(
            FOXGLOVE_TIMESTAMP,
            TimeType::TimestampNs,
            [Op::access_field("timestamp"), Op::time_spec_to_nanos()],
        )
        // TODO(grtlr): We leave out the `format` column because the `png` contents are not a valid MIME type.
        .component(
            EncodedImage::descriptor_blob(),
            [Op::access_field("data"), Op::binary_to_list_uint8()],
        )
    })
    .expect("failed to build CompressedImage lens")
    .build()
}

/// Creates a lens for `foxglove.PoseInFrame` messages (singular pose).
///
/// This lens transforms Foxglove `PoseInFrame` protobuf messages into Rerun `InstancePoses3D` components.
///
/// The Foxglove `PoseInFrame` message structure:
/// ```text
/// message PoseInFrame {
///   google.protobuf.Timestamp timestamp = 1;
///   string frame_id = 2;
///   Pose pose = 3;
/// }
///
/// message Pose {
///   Vector3 position = 1;
///   Quaternion orientation = 2;
/// }
/// ```
pub fn pose_in_frame_lens() -> Lens {
    let axes = transform_axes(1.0);

    Lens::for_input_column(EntityPathFilter::all(), "foxglove.PoseInFrame:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                TimeType::TimestampNs,
                [Op::access_field("timestamp"), Op::time_spec_to_nanos()],
            )
            .component(
                InstancePoses3D::descriptor_translations(),
                [
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
        })
        .expect("failed to build PoseInFrame lens")
        .output_static_columns(|out| {
            out.component(axes.descriptor.clone(), [Op::constant(axes.list_array.clone())])
        })
        .expect("failed to build PoseInFrame static columns")
        .build()
}

/// Creates a lens for `foxglove.PosesInFrame` messages (multiple poses).
///
/// This lens transforms Foxglove `PosesInFrame` protobuf messages into Rerun `InstancePoses3D` components.
///
/// The Foxglove `PosesInFrame` message structure:
/// ```text
/// message PosesInFrame {
///   google.protobuf.Timestamp timestamp = 1;
///   string frame_id = 2;
///   repeated Pose poses = 3;
/// }
/// ```
pub fn poses_in_frame_lens() -> Lens {
    let axes = transform_axes(0.1);

    Lens::for_input_column(EntityPathFilter::all(), "foxglove.PosesInFrame:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                TimeType::TimestampNs,
                [Op::access_field("timestamp"), Op::time_spec_to_nanos()],
            )
            .component(
                InstancePoses3D::descriptor_translations(),
                [
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
        })
        .expect("failed to build PosesInFrame lens")
        .output_static_columns(|out| {
            out.component(axes.descriptor.clone(), [Op::constant(axes.list_array.clone())])
        })
        .expect("failed to build PosesInFrame static columns")
        .build()
}

/// Creates a lens for `foxglove.CameraCalibration` messages.
///
/// This lens transforms Foxglove `CameraCalibration` protobuf messages into Rerun `Pinhole` components.
///
/// The Foxglove `CameraCalibration` message structure:
/// ```text
/// message CameraCalibration {
///   google.protobuf.Timestamp timestamp = 1;
///   string frame_id = 2;
///   uint32 width = 3;
///   uint32 height = 4;
///   string distortion_model = 5;
///   repeated double D = 6;  // distortion coefficients
///   repeated double K = 7;  // 3x3 intrinsic matrix (row-major)
///   repeated double R = 8;  // 3x3 rectification matrix
///   repeated double P = 9;  // 3x4 projection matrix
/// }
/// ```
pub fn camera_calibration_lens() -> Lens {
    Lens::for_input_column(
        EntityPathFilter::all(),
        "foxglove.CameraCalibration:message",
    )
    .output_columns(|out| {
        out.time(
            FOXGLOVE_TIMESTAMP,
            TimeType::TimestampNs,
            [Op::access_field("timestamp"), Op::time_spec_to_nanos()],
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
    })
    .expect("failed to build CameraCalibration lens")
    .build()
}

// =============================================================================
// Lens collection
// =============================================================================

/// Creates a collection of all Foxglove lenses.
///
/// Currently supports:
/// - `foxglove.FrameTransforms` -> `Transform3D`
/// - `foxglove.CompressedVideo` -> `VideoStream`
/// - `foxglove.CompressedImage` -> `EncodedImage`
/// - `foxglove.PoseInFrame` -> `InstancePoses3D` + `CoordinateFrame`
/// - `foxglove.PosesInFrame` -> `InstancePoses3D`
/// - `foxglove.CameraCalibration` -> `Pinhole`
///
/// More message types can be added in the future.
pub fn foxglove_lenses() -> Lenses {
    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    lenses.add_lens(frame_transforms_lens());
    lenses.add_lens(compressed_video_lens());
    lenses.add_lens(compressed_image_lens());
    lenses.add_lens(pose_in_frame_lens());
    lenses.add_lens(poses_in_frame_lens());
    lenses.add_lens(camera_calibration_lens());
    lenses
}
