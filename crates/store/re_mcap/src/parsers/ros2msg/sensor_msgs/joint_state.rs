use re_chunk::external::arrow::array::{Float64Builder, ListBuilder, StringBuilder};
use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, Scalars, SeriesLines};

use super::super::Ros2MessageParser;
use super::super::definitions::sensor_msgs;
use crate::Error;
use crate::parsers::{MessageParser, ParserContext, cdr};

pub struct JointStateMessageParser {
    joint_names: ListBuilder<StringBuilder>,
    positions: ListBuilder<Float64Builder>,
    velocities: ListBuilder<Float64Builder>,
    efforts: ListBuilder<Float64Builder>,
    frame_ids: ListBuilder<StringBuilder>,
}

impl Ros2MessageParser for JointStateMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            joint_names: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
            positions: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            velocities: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            efforts: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            frame_ids: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
        }
    }
}

impl MessageParser for JointStateMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let sensor_msgs::JointState {
            header,
            name,
            position,
            velocity,
            effort,
        } = cdr::try_decode_message::<sensor_msgs::JointState>(msg.data.as_ref())
            .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.values().append_value(header.frame_id);
        self.frame_ids.append(true);

        for name in &name {
            self.joint_names.values().append_value(name);
        }
        self.joint_names.append(true);

        self.positions.values().append_slice(position.as_slice());
        self.positions.append(true);

        self.velocities.values().append_slice(velocity.as_slice());
        self.velocities.append(true);

        self.efforts.values().append_slice(effort.as_slice());
        self.efforts.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            mut joint_names,
            mut positions,
            mut velocities,
            mut efforts,
            mut frame_ids,
        } = *self;

        let names_components = joint_names.finish();

        let positions_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone() / "position",
            timelines.clone(),
            [
                (Scalars::descriptor_scalars(), positions.finish()),
                (SeriesLines::descriptor_names(), names_components.clone()),
            ]
            .into_iter()
            .collect(),
        )?;

        let velocities_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone() / "velocity",
            timelines.clone(),
            [
                (Scalars::descriptor_scalars(), velocities.finish()),
                (SeriesLines::descriptor_names(), names_components.clone()),
            ]
            .into_iter()
            .collect(),
        )?;

        let efforts_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone() / "effort",
            timelines.clone(),
            [
                (Scalars::descriptor_scalars(), efforts.finish()),
                (SeriesLines::descriptor_names(), names_components.clone()),
            ]
            .into_iter()
            .collect(),
        )?;

        let frame_ids_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            std::iter::once((CoordinateFrame::descriptor_frame(), frame_ids.finish())).collect(),
        )?;

        Ok(vec![
            positions_chunk,
            velocities_chunk,
            efforts_chunk,
            frame_ids_chunk,
        ])
    }
}
