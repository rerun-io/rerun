use re_chunk::external::arrow::array::{
    FixedSizeListBuilder, Float64Builder, ListBuilder, StringBuilder,
};
use re_chunk::external::nohash_hasher::IntMap;
use re_chunk::{Chunk, ChunkId, ChunkResult, EntityPath, RowId, TimePoint};
use re_chunk::{TimeColumn, TimelineName};
use re_sdk_types::archetypes::{CoordinateFrame, Scalars, SeriesLines};
use re_sdk_types::reflection::ComponentDescriptorExt as _;
use re_sdk_types::ComponentDescriptor;

use super::super::definitions::sensor_msgs;
use super::super::Ros2MessageParser;
use crate::parsers::{cdr, MessageParser, ParserContext};
use crate::Error;

fn fixed_size_list_builder(
    value_length: i32,
    capacity: usize,
) -> FixedSizeListBuilder<Float64Builder> {
    FixedSizeListBuilder::with_capacity(Float64Builder::new(), value_length, capacity)
}

pub struct ImuMessageParser {
    orientation: FixedSizeListBuilder<Float64Builder>,
    sensor_readings: FixedSizeListBuilder<Float64Builder>,
    orientation_covariance: FixedSizeListBuilder<Float64Builder>,
    angular_velocity_covariance: FixedSizeListBuilder<Float64Builder>,
    linear_acceleration_covariance: FixedSizeListBuilder<Float64Builder>,
    orientation_stddev: FixedSizeListBuilder<Float64Builder>,
    angular_velocity_stddev: FixedSizeListBuilder<Float64Builder>,
    linear_acceleration_stddev: FixedSizeListBuilder<Float64Builder>,
    frame_ids: ListBuilder<StringBuilder>,
}

impl ImuMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.Imu";

    /// Helper function to create a metadata chunk for the Imu messages.
    fn metadata_chunk(entity_path: EntityPath) -> ChunkResult<Chunk> {
        Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &SeriesLines::new().with_names([
                    "gyroscope/x",
                    "gyroscope/y",
                    "gyroscope/z",
                    "accelerometer/x",
                    "accelerometer/y",
                    "accelerometer/z",
                ]),
            )
            .build()
    }

    fn stddev_metadata_chunk(
        entity_path: EntityPath,
        names: [&'static str; 3],
    ) -> ChunkResult<Chunk> {
        Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &SeriesLines::new().with_names(names),
            )
            .build()
    }

    fn covariance_stddevs(covariance: &[f64; 9]) -> [f64; 3] {
        // ROS uses all zeros for "unknown covariance" and a first value of -1 for
        // "this estimate is not provided". Avoid rendering these as confident zeros.
        if covariance[0] < 0.0 || covariance.iter().all(|value| *value == 0.0) {
            return [f64::NAN; 3];
        }

        [covariance[0], covariance[4], covariance[8]].map(|variance| {
            if variance >= 0.0 {
                variance.sqrt()
            } else {
                f64::NAN
            }
        })
    }

    fn stddev_chunk(
        entity_path: EntityPath,
        timelines: IntMap<TimelineName, TimeColumn>,
        mut stddevs: FixedSizeListBuilder<Float64Builder>,
    ) -> ChunkResult<Chunk> {
        Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            [(Scalars::descriptor_scalars(), stddevs.finish().into())]
                .into_iter()
                .collect(),
        )
    }
}

impl Ros2MessageParser for ImuMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            orientation: fixed_size_list_builder(4, num_rows),
            sensor_readings: fixed_size_list_builder(6, num_rows),
            orientation_covariance: fixed_size_list_builder(9, num_rows),
            angular_velocity_covariance: fixed_size_list_builder(9, num_rows),
            linear_acceleration_covariance: fixed_size_list_builder(9, num_rows),
            orientation_stddev: fixed_size_list_builder(3, num_rows),
            angular_velocity_stddev: fixed_size_list_builder(3, num_rows),
            linear_acceleration_stddev: fixed_size_list_builder(3, num_rows),
            frame_ids: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
        }
    }
}

impl MessageParser for ImuMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let imu =
            cdr::try_decode_message::<sensor_msgs::Imu>(msg.data.as_ref()).map_err(Error::other)?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::from_nanos_ros2(
            imu.header.stamp.as_nanos() as u64,
            ctx.time_type(),
        ));

        self.frame_ids.values().append_value(imu.header.frame_id);
        self.frame_ids.append(true);

        self.orientation.values().append_slice(&[
            imu.orientation.x,
            imu.orientation.y,
            imu.orientation.z,
            imu.orientation.w,
        ]);

        self.sensor_readings.values().append_slice(&[
            imu.angular_velocity.x,
            imu.angular_velocity.y,
            imu.angular_velocity.z,
            imu.linear_acceleration.x,
            imu.linear_acceleration.y,
            imu.linear_acceleration.z,
        ]);

        self.orientation_covariance
            .values()
            .append_slice(&imu.orientation_covariance);
        self.angular_velocity_covariance
            .values()
            .append_slice(&imu.angular_velocity_covariance);
        self.linear_acceleration_covariance
            .values()
            .append_slice(&imu.linear_acceleration_covariance);

        self.orientation_stddev
            .values()
            .append_slice(&Self::covariance_stddevs(&imu.orientation_covariance));
        self.angular_velocity_stddev
            .values()
            .append_slice(&Self::covariance_stddevs(&imu.angular_velocity_covariance));
        self.linear_acceleration_stddev
            .values()
            .append_slice(&Self::covariance_stddevs(
                &imu.linear_acceleration_covariance,
            ));

        self.orientation.append(true);
        self.sensor_readings.append(true);
        self.orientation_covariance.append(true);
        self.angular_velocity_covariance.append(true);
        self.linear_acceleration_covariance.append(true);
        self.orientation_stddev.append(true);
        self.angular_velocity_stddev.append(true);
        self.linear_acceleration_stddev.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();
        let meta_chunk = Self::metadata_chunk(entity_path.clone())?;

        let Self {
            mut orientation,
            mut sensor_readings,
            mut orientation_covariance,
            mut angular_velocity_covariance,
            mut linear_acceleration_covariance,
            orientation_stddev,
            angular_velocity_stddev,
            linear_acceleration_stddev,
            mut frame_ids,
        } = *self;

        let orientation_stddev_path =
            entity_path.join(&EntityPath::from("covariance/orientation_stddev"));
        let angular_velocity_stddev_path =
            entity_path.join(&EntityPath::from("covariance/angular_velocity_stddev"));
        let linear_acceleration_stddev_path =
            entity_path.join(&EntityPath::from("covariance/linear_acceleration_stddev"));

        let data_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            [
                (
                    Scalars::descriptor_scalars(),
                    sensor_readings.finish().into(),
                ),
                (
                    // TODO(#10727): Figure out why logging this as `Transform3D.quaternion` doesn't work.
                    ComponentDescriptor::partial("orientation")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    orientation.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("orientation_covariance")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    orientation_covariance.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("angular_velocity_covariance")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    angular_velocity_covariance.finish().into(),
                ),
                (
                    ComponentDescriptor::partial("linear_acceleration_covariance")
                        .with_builtin_archetype(Self::ARCHETYPE_NAME),
                    linear_acceleration_covariance.finish().into(),
                ),
                (CoordinateFrame::descriptor_frame(), frame_ids.finish()),
            ]
            .into_iter()
            .collect(),
        )?;

        Ok(vec![
            data_chunk,
            meta_chunk,
            Self::stddev_chunk(
                orientation_stddev_path.clone(),
                timelines.clone(),
                orientation_stddev,
            )?,
            Self::stddev_metadata_chunk(orientation_stddev_path, ["roll", "pitch", "yaw"])?,
            Self::stddev_chunk(
                angular_velocity_stddev_path.clone(),
                timelines.clone(),
                angular_velocity_stddev,
            )?,
            Self::stddev_metadata_chunk(angular_velocity_stddev_path, ["x", "y", "z"])?,
            Self::stddev_chunk(
                linear_acceleration_stddev_path.clone(),
                timelines,
                linear_acceleration_stddev,
            )?,
            Self::stddev_metadata_chunk(linear_acceleration_stddev_path, ["x", "y", "z"])?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::ImuMessageParser;

    #[test]
    fn covariance_stddevs_use_diagonal_variances() {
        let covariance = [0.04, 0.01, 0.02, 0.01, 0.09, 0.03, 0.02, 0.03, 0.16];

        assert_eq!(
            ImuMessageParser::covariance_stddevs(&covariance),
            [0.2, 0.3, 0.4]
        );
    }

    #[test]
    fn covariance_stddevs_hide_ros_unknown_covariance() {
        let stddevs = ImuMessageParser::covariance_stddevs(&[0.0; 9]);
        assert!(stddevs.iter().all(|value| value.is_nan()));
    }

    #[test]
    fn covariance_stddevs_hide_ros_missing_estimate() {
        let mut covariance = [0.0; 9];
        covariance[0] = -1.0;

        let stddevs = ImuMessageParser::covariance_stddevs(&covariance);
        assert!(stddevs.iter().all(|value| value.is_nan()));
    }
}
