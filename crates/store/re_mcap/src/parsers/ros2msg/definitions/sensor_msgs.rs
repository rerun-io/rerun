//! Definitions for the ROS2 `sensor_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/sensor_msgs>

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::geometry_msgs;
use super::std_msgs::Header;

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
#[serde(try_from = "u8", into = "u8")]
#[repr(u8)]
pub enum PointFieldDatatype {
    Int8 = 1,
    UInt8 = 2,
    Int16 = 3,
    UInt16 = 4,
    Int32 = 5,
    UInt32 = 6,
    Float32 = 7,
    Float64 = 8,
}

#[derive(Debug, thiserror::Error)]
#[error("unknown point field datatype: {0}")]
pub struct UnknownPointFieldDatatype(u8);

impl TryFrom<u8> for PointFieldDatatype {
    type Error = UnknownPointFieldDatatype;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Int8,
            2 => Self::UInt8,
            3 => Self::Int16,
            4 => Self::UInt16,
            5 => Self::Int32,
            6 => Self::UInt32,
            7 => Self::Float32,
            8 => Self::Float64,
            other => Err(UnknownPointFieldDatatype(other))?,
        })
    }
}

impl From<PointFieldDatatype> for u8 {
    fn from(datatype: PointFieldDatatype) -> Self {
        datatype as Self
    }
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

    /// True if there are no invalid points
    pub is_dense: bool,
}

/// This message is used to specify a region of interest in an image.
///
/// When used to specify the ROI setting of the camera when the image was taken, the `height` and `width`
/// should be the same as the height and width of the image.
#[derive(Debug, Serialize, Deserialize)]
pub struct RegionOfInterest {
    /// The x-coordinate of the top-left corner of the region.
    pub x_offset: u32,

    /// The y-coordinate of the top-left corner of the region.
    pub y_offset: u32,

    /// The height of the region.
    pub height: u32,

    /// The width of the region.
    pub width: u32,

    /// Whether the region is active (true) or inactive (false).
    pub do_rectify: bool,
}

/// This message contains information about a camera, such as its intrinsic parameters.
#[derive(Debug, Serialize, Deserialize)]
pub struct CameraInfo {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    /// The height of the image in pixels.
    pub height: u32,

    /// The width of the image in pixels.
    pub width: u32,

    /// The distortion model used. Supported models are listed in
    /// `sensor_msgs/distortion_models.h`.
    ///
    /// For most cameras, `plumb_bob` - a simple model of radial and tangential distortion - is sufficient.
    pub distortion_model: String,

    /// The distortion parameters, size depending on the distortion model.
    ///
    /// E.g. For `plumb_bob`, the 5 parameters are: (k1, k2, t1, t2, k3),
    /// and for `kannala_brandt` the parameters are (k1, k2, k3, k4)
    pub d: Vec<f64>,

    /// The intrinsic camera matrix for the raw (distorted) images.
    ///
    /// Projects 3D points in the camera coordinate frame to 2D pixel
    /// coordinates using the focal lengths (fx, fy) and principal point (cx, cy).
    pub k: [f64; 9],

    /// Rectification matrix (stereo cameras only)
    ///
    /// A rotation matrix aligning the camera coordinate system to the ideal stereo image plane
    /// so that the epipolar lines in both stereo images are parallel.
    pub r: [f64; 9],

    /// Projection/camera matrix
    ///
    /// By convention, this matrix specifies the intrinsic (camera) matrix of the processed (rectified) image.
    /// That is, the left 3x3 portion is the normal camera intrinsic matrix for the rectified image.
    ///
    /// It projects 3D points in the camera coordinate frame to 2D pixel
    /// coordinates using the focal lengths (fx', fy') and principal point
    /// (cx', cy') - these may differ from the values in K.
    pub p: [f64; 12],

    /// Binning refers here to any camera setting which combines rectangular
    /// neighborhoods of pixels into larger "super-pixels." It reduces the
    /// resolution of the output image to
    /// (`width` / `binning_x`) x (`height` / `binning_y`).
    pub binning_x: u32,
    pub binning_y: u32,

    /// Region of interest (subwindow of full camera resolution), given in
    /// full resolution (unbinned) image coordinates. A particular ROI
    /// always denotes the same window of pixels on the camera sensor,
    /// regardless of binning settings.
    pub roi: RegionOfInterest,
}

/// This is a message that holds data to describe the state of a set of torque controlled joints.
///
/// The state of each joint (revolute or prismatic) is defined by:
/// * the position of the joint (rad or m),
/// * the velocity of the joint (rad/s or m/s) and
/// * the effort that is applied in the joint (Nm or N).
///
/// Each joint is uniquely identified by its name
/// The header specifies the time at which the joint states were recorded. All the joint states
/// in one message have to be recorded at the same time.
///
/// This message consists of a multiple arrays, one for each part of the joint state.
/// The goal is to make each of the fields optional. When e.g. your joints have no
/// effort associated with them, you can leave the effort array empty.
///
/// All arrays in this message should have the same size, or be empty.
/// This is the only way to uniquely associate the joint name with the correct
/// states.
#[derive(Debug, Serialize, Deserialize)]
pub struct JointState {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    /// The names of the joints.
    pub name: Vec<String>,

    /// The positions of the joints.
    pub position: Vec<f64>,

    /// The velocities of the joints.
    pub velocity: Vec<f64>,

    /// The efforts applied in the joints.
    pub effort: Vec<f64>,
}

/// Navigation Satellite fix status information.
#[derive(Debug, Serialize, Deserialize)]
pub struct NavSatStatus {
    /// Navigation satellite fix status.
    pub status: NavSatFixStatus,

    /// Navigation satellite service type.
    pub service: NavSatService,
}

/// Navigation satellite fix status values.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "i8", into = "i8")]
#[repr(i8)]
pub enum NavSatFixStatus {
    /// Status is unknown.
    Unknown = -2,

    /// Unable to fix position.
    NoFix = -1,

    /// Unaugmented fix.
    Fix = 0,

    /// Satellite-based augmentation.
    SbasFix = 1,

    /// Ground-based augmentation.
    GbasFix = 2,
}

impl From<i8> for NavSatFixStatus {
    fn from(value: i8) -> Self {
        match value {
            -1 => Self::NoFix,
            0 => Self::Fix,
            1 => Self::SbasFix,
            2 => Self::GbasFix,
            _ => Self::Unknown,
        }
    }
}

impl From<NavSatFixStatus> for i8 {
    fn from(status: NavSatFixStatus) -> Self {
        status as Self
    }
}

/// Navigation satellite service type values.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u16", into = "u16")]
#[repr(u16)]
pub enum NavSatService {
    Unknown = 0,
    Gps = 1,
    Glonass = 2,
    Compass = 4,
    Galileo = 8,
}

impl From<u16> for NavSatService {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::Gps,
            2 => Self::Glonass,
            4 => Self::Compass,
            8 => Self::Galileo,
            _ => Self::Unknown,
        }
    }
}

impl From<NavSatService> for u16 {
    fn from(service: NavSatService) -> Self {
        service as Self
    }
}

/// Position covariance type for navigation satellite fix.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
#[repr(u8)]
pub enum CovarianceType {
    Unknown = 0,
    Approximated = 1,
    DiagonalKnown = 2,
    Known = 3,
}

impl From<u8> for CovarianceType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Approximated,
            2 => Self::DiagonalKnown,
            3 => Self::Known,
            _ => Self::Unknown,
        }
    }
}

impl From<CovarianceType> for u8 {
    fn from(cov_type: CovarianceType) -> Self {
        cov_type as Self
    }
}

/// Navigation Satellite fix for any Global Navigation Satellite System
///
/// Specified using the WGS 84 reference ellipsoid.
///
/// `header.stamp` specifies the ROS time for this measurement (the
/// corresponding satellite time may be reported using the
/// `sensor_msgs/TimeReference` message).
///
/// `header.frame_id` is the frame of reference reported by the satellite
/// receiver, usually the location of the antenna. This is a
/// Euclidean frame relative to the vehicle, not a reference
/// ellipsoid.
#[derive(Debug, Serialize, Deserialize)]
pub struct NavSatFix {
    /// Metadata including timestamp and coordinate frame.
    pub header: Header,

    /// Satellite fix status information.
    pub status: NavSatStatus,

    /// Latitude (degrees). Positive is north of equator; negative is south.
    pub latitude: f64,

    /// Longitude (degrees). Positive is east of prime meridian; negative is west.
    pub longitude: f64,

    /// Altitude (m). Positive is above the WGS 84 ellipsoid
    /// (quiet NaN if no altitude is available).
    pub altitude: f64,

    /// Position covariance (m^2) defined relative to a tangential plane
    /// through the reported position. The components are East, North, and
    /// Up (ENU), in row-major order.
    ///
    /// Beware: this coordinate system exhibits singularities at the poles.
    pub position_covariance: [f64; 9],

    /// If the covariance of the fix is known, fill it in completely. If the
    /// GPS receiver provides the variance of each measurement, put them
    /// along the diagonal. If only Dilution of Precision is available,
    /// estimate an approximate covariance from that.
    pub position_covariance_type: CovarianceType,
}

/// A single temperature reading.
#[derive(Debug, Serialize, Deserialize)]
pub struct Temperature {
    /// Timestamp is the time the temperature was measured.
    /// `frame_id` is the location of the temperature reading.
    pub header: Header,

    /// Measurement of the Temperature in Degrees Celsius.
    pub temperature: f64,

    /// 0 is interpreted as variance unknown.
    pub variance: f64,
}

/// Single pressure reading for fluids (air, water, etc).
///
/// This message is appropriate for measuring the pressure inside of a fluid (air, water, etc).
/// This also includes atmospheric or barometric pressure.
/// This message is not appropriate for force/pressure contact sensors.
#[derive(Debug, Serialize, Deserialize)]
pub struct FluidPressure {
    /// Timestamp of the measurement.
    /// `frame_id` is the location of the pressure sensor.
    pub header: Header,

    /// Absolute pressure reading in Pascals.
    pub fluid_pressure: f64,

    /// 0 is interpreted as variance unknown.
    pub variance: f64,
}

/// Single reading from a relative humidity sensor.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelativeHumidity {
    /// Timestamp is the time the humidity was measured.
    /// `frame_id` is the location of the humidity sensor.
    pub header: Header,

    /// Expression of the relative humidity from `0.0` to `1.0`.
    ///
    /// - `0.0` is no partial pressure of water vapor
    /// - `1.0` represents partial pressure of saturation
    pub humidity: f64,

    /// 0 is interpreted as variance unknown.
    pub variance: f64,
}

/// Single photometric illuminance measurement.
///
/// Light should be assumed to be measured along the sensor's x-axis (the area of detection is the y-z plane).
/// The illuminance should have a 0 or positive value and be received with
/// the sensor's +X axis pointing toward the light source.
///
/// Photometric illuminance is the measure of the human eye's sensitivity of the
/// intensity of light encountering or passing through a surface.
///
/// All other Photometric and Radiometric measurements should not use this message.
///
/// This message cannot represent:
///  - Luminous intensity (candela/light source output)
///  - Luminance (nits/light output per area)
///  - Irradiance (watt/area), etc.
#[derive(Debug, Serialize, Deserialize)]
pub struct Illuminance {
    /// Timestamp is the time the illuminance was measured.
    /// `frame_id` is the location of the illuminance sensor.
    pub header: Header,

    /// Measurement of the Photometric Illuminance in Lux.
    pub illuminance: f64,

    /// 0 is interpreted as variance unknown.
    pub variance: f64,
}

/// Radiation type for range sensors.
/// 0 = ULTRASOUND, 1 = INFRARED
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
#[repr(u8)]
pub enum RadiationType {
    Ultrasound = 0,
    Infrared = 1,
}

impl From<u8> for RadiationType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Infrared,
            _ => Self::Ultrasound,
        }
    }
}

impl From<RadiationType> for u8 {
    fn from(radiation_type: RadiationType) -> Self {
        radiation_type as Self
    }
}

/// Single range reading from an active ranger that emits energy and reports
/// one range reading that is valid along an arc at the distance measured.
///
/// This message is not appropriate for laser scanners.
///
/// Supports both modern and legacy formats - the variance field is optional for backward compatibility.
#[derive(Debug, Serialize, Deserialize)]
pub struct Range {
    pub header: Header,

    /// The type of radiation used by the sensor.
    pub radiation_type: RadiationType,

    /// The size of the arc that the distance reading is valid for (rad).
    pub field_of_view: f32,

    /// Minimum range value (m).
    pub min_range: f32,

    /// Maximum range value (m).
    pub max_range: f32,

    /// Range data (m).
    ///
    /// ### Note
    ///
    /// This message can also represent a binary sensor that will output -Inf
    /// if the object is detected and +Inf if the object is outside of detection range).
    pub range: f32,
}

/// Power supply status values.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
#[repr(u8)]
pub enum PowerSupplyStatus {
    Unknown = 0,
    Charging = 1,
    Discharging = 2,
    NotCharging = 3,
    Full = 4,
}

impl From<u8> for PowerSupplyStatus {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::NotCharging,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

impl From<PowerSupplyStatus> for u8 {
    fn from(status: PowerSupplyStatus) -> Self {
        status as Self
    }
}

/// Power supply health values.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
#[repr(u8)]
pub enum PowerSupplyHealth {
    Unknown = 0,
    Good = 1,
    Overheat = 2,
    Dead = 3,
    Overvoltage = 4,
    UnspecFailure = 5,
    Cold = 6,
    WatchdogTimerExpire = 7,
    SafetyTimerExpire = 8,
}

impl From<u8> for PowerSupplyHealth {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Good,
            2 => Self::Overheat,
            3 => Self::Dead,
            4 => Self::Overvoltage,
            5 => Self::UnspecFailure,
            6 => Self::Cold,
            7 => Self::WatchdogTimerExpire,
            8 => Self::SafetyTimerExpire,
            _ => Self::Unknown,
        }
    }
}

impl From<PowerSupplyHealth> for u8 {
    fn from(health: PowerSupplyHealth) -> Self {
        health as Self
    }
}

/// Power supply technology values.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
#[repr(u8)]
pub enum PowerSupplyTechnology {
    Unknown = 0,
    Nimh = 1,
    Lion = 2,
    Lipo = 3,
    Life = 4,
    Nicd = 5,
    Limn = 6,
    Ternary = 7,
    Vrla = 8,
}

impl From<u8> for PowerSupplyTechnology {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Nimh,
            2 => Self::Lion,
            3 => Self::Lipo,
            4 => Self::Life,
            5 => Self::Nicd,
            6 => Self::Limn,
            7 => Self::Ternary,
            8 => Self::Vrla,
            _ => Self::Unknown,
        }
    }
}

impl From<PowerSupplyTechnology> for u8 {
    fn from(tech: PowerSupplyTechnology) -> Self {
        tech as Self
    }
}

/// Describes the power state of the battery.
///
/// Constants are chosen to match the enums in the linux kernel
/// defined in `include/linux/power_supply.h` as of version 3.7
///
/// The percentage value should not be trusted if it is exactly 0 or 100.
/// Only use as a hint for UI applications.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatteryState {
    pub header: Header,

    /// Voltage in Volts (Mandatory).
    pub voltage: f32,

    /// Temperature in Degrees Celsius (If unmeasured NaN).
    pub temperature: f32,

    /// Negative when discharging (A).
    pub current: f32,

    /// Current charge in Ah (If unmeasured NaN).
    pub charge: f32,

    /// Capacity in Ah (last full capacity) (If unmeasured NaN).
    pub capacity: f32,

    /// Capacity in Ah (design capacity) (If unmeasured NaN).
    pub design_capacity: f32,

    /// Charge percentage on 0 to 1 range (If unmeasured NaN).
    pub percentage: f32,

    /// The charging status as reported. Values defined above.
    pub power_supply_status: PowerSupplyStatus,

    /// The battery health metric. Values defined above.
    pub power_supply_health: PowerSupplyHealth,

    /// The battery chemistry. Values defined above.
    pub power_supply_technology: PowerSupplyTechnology,

    /// True if the battery is present.
    pub present: bool,

    /// An array of individual cell voltages for each cell in the pack
    /// If individual voltages unknown but number of cells known set each to NaN.
    pub cell_voltage: Vec<f32>,

    /// An array of individual cell temperatures for each cell in the pack
    /// If individual temperatures unknown but number of cells known set each to NaN.
    pub cell_temperature: Vec<f32>,

    /// The location into which the battery is inserted. (slot number or plug).
    pub location: String,

    /// The best approximation of the battery serial number.
    pub serial_number: String,
}

/// Measurement of the Magnetic Field vector at a specific location.
///
/// If the covariance of the measurement is known, it should be filled in.
/// If all you know is the variance of each measurement, e.g. from the datasheet,
/// just put those along the diagonal.
///
/// A covariance matrix of all zeros will be interpreted as "covariance unknown",
/// and to use the data a covariance will have to be assumed or gotten from some
/// other source.
#[derive(Debug, Serialize, Deserialize)]
pub struct MagneticField {
    /// Timestamp is the time the field was measured.
    /// `frame_id` is the location and orientation of the field measurement.
    pub header: Header,

    /// X, Y, and Z components of the field vector in Tesla.
    /// If your sensor does not output 3 axes, put `NaNs` in the components not reported.
    pub magnetic_field: geometry_msgs::Vector3,

    /// Row major about x, y, z axes.
    /// 0 is interpreted as variance unknown.
    pub magnetic_field_covariance: [f64; 9],
}
