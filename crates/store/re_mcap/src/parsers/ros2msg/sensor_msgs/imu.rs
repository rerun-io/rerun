use re_chunk::external::arrow::array::{
    FixedSizeListBuilder, Float64Builder, ListBuilder, StringBuilder,
};
use re_chunk::{Chunk, ChunkId, ChunkResult, EntityPath, RowId, TimePoint};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::{CoordinateFrame, Scalars, SeriesLines};
use re_sdk_types::reflection::ComponentDescriptorExt as _;

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use crate::Error;
use crate::parsers::{MessageParser, ParserContext, cdr};

fn fixed_size_list_builder(
    value_length: i32,
    capacity: usize,
) -> FixedSizeListBuilder<Float64Builder> {
    FixedSizeListBuilder::with_capacity(Float64Builder::new(), value_length, capacity)
}

pub struct ImuMessageParser {
    orientation: FixedSizeListBuilder<Float64Builder>,
    sensor_readings: FixedSizeListBuilder<Float64Builder>,
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
}

impl Ros2MessageParser for ImuMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            orientation: fixed_size_list_builder(4, num_rows),
            sensor_readings: fixed_size_list_builder(6, num_rows),
            frame_ids: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
        }
    }
}

impl MessageParser for ImuMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let imu = cdr::try_decode_message::<sensor_msgs::Imu>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            imu.header.stamp.as_nanos() as u64,
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

        self.orientation.append(true);
        self.sensor_readings.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();
        let meta_chunk = Self::metadata_chunk(entity_path.clone())?;

        let Self {
            mut orientation,
            mut sensor_readings,
            mut frame_ids,
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
                (CoordinateFrame::descriptor_frame(), frame_ids.finish()),
                // TODO(#10728): Figure out what to do with the covariance matrices.
            ]
            .into_iter()
            .collect(),
        )?;

        Ok(vec![data_chunk, meta_chunk])
    }
}
