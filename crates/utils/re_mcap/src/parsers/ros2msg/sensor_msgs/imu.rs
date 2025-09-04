use super::super::definitions::sensor_msgs;
use re_chunk::{
    Chunk, ChunkId, ChunkResult, EntityPath, RowId, TimePoint,
    external::arrow::array::{FixedSizeListBuilder, Float64Builder},
};
use re_log_types::TimeCell;
use re_types::{
    ComponentDescriptor,
    archetypes::{Scalars, SeriesLines},
    reflection::ComponentDescriptorExt as _,
};

use super::super::Ros2MessageParser;
use crate::{
    Error,
    parsers::{MessageParser, ParserContext, cdr},
};

/// Plugin that parses `sensor_msgs/msg/Imu` messages.
#[derive(Default)]
pub struct ImuSchemaPlugin;

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
}

impl Ros2MessageParser for ImuMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            orientation: fixed_size_list_builder(4, num_rows),
            sensor_readings: fixed_size_list_builder(6, num_rows),
            orientation_covariance: fixed_size_list_builder(9, num_rows),
            angular_velocity_covariance: fixed_size_list_builder(9, num_rows),
            linear_acceleration_covariance: fixed_size_list_builder(9, num_rows),
        }
    }
}

impl MessageParser for ImuMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let imu = cdr::try_decode_message::<sensor_msgs::Imu>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(imu.header.stamp.as_nanos()),
        );

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

        self.orientation.append(true);
        self.sensor_readings.append(true);
        self.orientation_covariance.append(true);
        self.angular_velocity_covariance.append(true);
        self.linear_acceleration_covariance.append(true);

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
        } = *self;

        let data_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
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
                // TODO(#10728): Figure out what to do with the covariance matrices.
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
            ]
            .into_iter()
            .collect(),
        )?;

        Ok(vec![data_chunk, meta_chunk])
    }
}
