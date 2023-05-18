use crate::{Component, EntityPath};
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use super::{Point3D, Quaternion};

#[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct ImuData {
    pub accel: Point3D,
    pub gyro: Point3D,
    pub mag: Option<Point3D>,
    pub orientation: Quaternion,
}

impl ImuData {
    pub fn entity_path() -> EntityPath {
        "imu_data".into()
    }
}

impl Component for ImuData {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.imu".into()
    }
}
