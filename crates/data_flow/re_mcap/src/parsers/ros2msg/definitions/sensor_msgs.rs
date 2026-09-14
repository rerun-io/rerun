//! Definitions for the ROS2 `sensor_msgs` package.
//!
//! Based on definitions taken from <https://github.com/ros2/common_interfaces/tree/rolling/sensor_msgs>

use serde::{Deserialize, Serialize};

use super::geometry_msgs;
use super::std_msgs::Header;

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

/// Reports the state of a joystick's axes and buttons.
#[derive(Debug, Serialize, Deserialize)]
pub struct Joy {
    /// Timestamp in the header is the time the data was received from the joystick.
    pub header: Header,

    /// The axes measurements from a joystick.
    pub axes: Vec<f32>,

    /// The buttons measurements from a joystick.
    pub buttons: Vec<i32>,
}
