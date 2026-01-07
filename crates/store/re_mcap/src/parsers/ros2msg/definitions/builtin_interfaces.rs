//! Definitions for the ROS2 `builtin_interfaces` package.
//!
//! Based on definitions taken from <https://github.com/ros2/rcl_interfaces/tree/rolling/builtin_interfaces/msg>

use serde::{Deserialize, Serialize};

/// Represents a specific point in ROS Time.
///
/// Messages of this datatype follow the ROS Time design:
/// <https://design.ros2.org/articles/clock_and_time.html>
///
/// # Examples
/// - The time `-1.7` seconds is represented as `{ sec: -2, nanosec: 300_000_000 }`
/// - The time `1.7` seconds is represented as `{ sec: 1,  nanosec: 700_000_000 }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Time {
    /// The seconds component, valid over all `int32` values.
    pub sec: i32,

    /// The nanoseconds component, valid in the range `[0, 1_000_000_000)`.
    /// This is added to the seconds component.
    pub nanosec: u32,
}

impl Time {
    /// Converts the time to total nanoseconds as a signed 64-bit integer.
    pub fn as_nanos(&self) -> i64 {
        (self.sec as i64) * 1_000_000_000 + (self.nanosec as i64)
    }

    /// Converts the time to whole seconds, truncating any fractional part.
    #[cfg_attr(not(test), expect(dead_code))] // only used in tests
    pub fn as_secs(&self) -> i64 {
        (self.sec as i64) + (self.nanosec as i64) / 1_000_000_000
    }

    /// Converts the time to seconds as a [`f64`].
    #[cfg_attr(not(test), expect(dead_code))] // only used in tests
    pub fn as_secs_f64(&self) -> f64 {
        self.sec as f64 + (self.nanosec as f64) / 1_000_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_nanos() {
        let time = Time {
            sec: 1,
            nanosec: 700_000_000,
        };
        assert_eq!(time.as_nanos(), 1_700_000_000);

        let time = Time {
            sec: -2,
            nanosec: 300_000_000,
        };
        assert_eq!(time.as_nanos(), -1_700_000_000);
    }

    #[test]
    fn test_as_secs() {
        let time = Time {
            sec: 1,
            nanosec: 700_000_000,
        };
        assert_eq!(time.as_secs(), 1);
        assert_eq!(time.as_secs_f64(), 1.7);

        let time = Time {
            sec: -2,
            nanosec: 300_000_000,
        };
        assert_eq!(time.as_secs(), -2);
        assert_eq!(time.as_secs_f64(), -1.7);
    }
}
