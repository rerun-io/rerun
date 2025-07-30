//! Definitions for the ROS2 `sensor_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/sensor_msgs>

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{geometry_msgs, std_msgs::Header};

/// This message contains an uncompressed image.
///
/// The pixel at coordinate (0, 0) is at the top-left corner of the image.
#[derive(Debug, Serialize, Deserialize)]
pub struct Image<'a> {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    /// Image height in pixels (number of rows).
    pub height: u32,

    /// Image width in pixels (number of columns).
    pub width: u32,

    /// Encoding of the pixel data (e.g., `rgb8`, `mono8`, `bgr16`, etc.).
    ///
    /// Taken from the list of strings in [include/sensor_msgs/image_encodings](https://github.com/ros2/common_interfaces/blob/rolling/sensor_msgs/include/sensor_msgs/image_encodings.hpp)
    pub encoding: String,

    /// Whether the data is big-endian.
    pub is_bigendian: u8,

    /// Full row length in bytes.
    pub step: u32,

    #[serde(with = "serde_bytes")]
    #[serde(borrow)]
    /// Actual pixel data matrix, size is `step * height` bytes.
    pub data: Cow<'a, [u8]>,
}

/// This message contains a compressed image.
///
/// `format` encodes the compression scheme and pixel format, and must be interpreted
/// according to the transport used (e.g., `compressed_image_transport`, `compressed_depth_image_transport`, etc.).
///
/// # Format rules
///
/// ### `compressed_image_transport`
/// - Format: `ORIG_PIXFMT; CODEC [COMPRESSED_PIXFMT]`
/// - `ORIG_PIXFMT`: e.g., `rgb8`, `mono8`, etc.
/// - `CODEC`: `jpeg` or `png`
/// - `COMPRESSED_PIXFMT` (for color images only):
///     - JPEG: `bgr8`, `rgb8`
///     - PNG: `bgr8`, `rgb8`, `bgr16`, `rgb16`
///
/// If the field is empty or doesn't match, assume a `bgr8` or `mono8` JPEG.
///
/// ### `compressed_depth_image_transport`
///
/// - Format: `ORIG_PIXFMT; compressedDepth CODEC`
/// - `ORIG_PIXFMT`: typically `16UC1` or `32FC1`
/// - `CODEC`: `png` or `rvl`
///
/// If the field is empty or doesn't match, assume a PNG image.
///
/// ### Other Transports
///
/// Users may define their own formats.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompressedImage<'a> {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    /// Format string indicating codec and pixel format. See format rules above.
    pub format: String,

    #[serde(with = "serde_bytes")]
    #[serde(borrow)]
    /// Byte buffer containing the compressed image.
    pub data: Cow<'a, [u8]>,
}

/// This is a message to hold data from an IMU (Inertial Measurement Unit)
///
/// Accelerations should be in m/s^2 (not in g's), and rotational velocity should be in rad/sec
///
/// If the covariance of the measurement is known, it should be filled in (if all you know is the
/// variance of each measurement, e.g. from the datasheet, just put those along the diagonal)
/// A covariance matrix of all zeros will be interpreted as "covariance unknown", and to use the
/// data a covariance will have to be assumed or gotten from some other source
///
/// If you have no estimate for one of the data elements (e.g. your IMU doesn't produce an
/// orientation estimate), please set element 0 of the associated covariance matrix to -1
/// If you are interpreting this message, please check for a value of -1 in the first element of each
/// covariance matrix, and disregard the associated estimate.
#[derive(Debug, Serialize, Deserialize)]
pub struct Imu {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    pub orientation: geometry_msgs::Quaternion,
    pub orientation_covariance: [f64; 9],

    pub angular_velocity: geometry_msgs::Vector3,
    pub angular_velocity_covariance: [f64; 9],

    pub linear_acceleration: geometry_msgs::Vector3,
    pub linear_acceleration_covariance: [f64; 9],
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum PointFieldDatatype {
    /// Does not exist in original spec.
    Unknown = 0,
    Int8 = 1,
    UInt8 = 2,
    Int16 = 3,
    UInt16 = 4,
    Int32 = 5,
    UInt32 = 6,
    Float32 = 7,
    Float64 = 8,
}

/// This message holds the description of one point entry in the
/// [`PointCloud2`] message format.
#[derive(Debug, Serialize, Deserialize)]
pub struct PointField {
    /// Common [`PointField`] names are x, y, z, intensity, rgb, rgba
    pub name: String,
    pub offset: u32,
    pub datatype: PointFieldDatatype,
    pub count: u32,
}

/// This message holds a collection of N-dimensional points.
///
/// It may contain additional information such as normals, intensity, etc. The
/// point data is stored as a binary blob, its layout described by the
/// contents of the "fields" array.
///
/// The point cloud data may be organized 2D (image-like) or 1D (unordered).
/// Point clouds organized as 2D images may be produced by camera depth sensors
/// such as stereo or time-of-flight.
#[derive(Debug, Serialize, Deserialize)]
pub struct PointCloud2 {
    /// Time of sensor data acquisition, and the coordinate frame ID (for 3D points).
    pub header: Header,

    /// 2D structure of the point cloud. If the cloud is unordered, height is
    /// 1 and width is the length of the point cloud.
    pub height: u32,
    pub width: u32,

    /// Describes the channels and their layout in the binary data blob.
    pub fields: Vec<PointField>,

    /// Is this data bigendian?
    pub is_bigendian: bool,

    /// Length of a point in bytes
    pub point_step: u32,

    /// Length of a row in bytes
    pub row_step: u32,

    /// Actual point data, size is (`row_step`*`height`)
    pub data: Vec<u8>,
}
