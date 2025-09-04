use super::super::definitions::sensor_msgs;
use re_chunk::{
    Chunk, ChunkId,
    external::arrow::array::{Float64Builder, ListBuilder, StringBuilder},
};
use re_log_types::TimeCell;
use re_types::archetypes::{Scalars, SeriesLines};

use super::super::Ros2MessageParser;
use crate::{
    Error,
    parsers::{MessageParser, ParserContext, cdr},
};

/// Plugin that parses `sensor_msgs/msg/JointState` messages.
#[derive(Default)]
pub struct JointStateSchemaPlugin;

pub struct JointStateMessageParser {
    joint_names: ListBuilder<StringBuilder>,
    positions: ListBuilder<Float64Builder>,
    velocities: ListBuilder<Float64Builder>,
    efforts: ListBuilder<Float64Builder>,
}

impl Ros2MessageParser for JointStateMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            joint_names: ListBuilder::with_capacity(StringBuilder::new(), num_rows),
            positions: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            velocities: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
            efforts: ListBuilder::with_capacity(Float64Builder::new(), num_rows),
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
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(header.stamp.as_nanos()),
        );

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
            timelines,
            [
                (Scalars::descriptor_scalars(), efforts.finish()),
                (SeriesLines::descriptor_names(), names_components.clone()),
            ]
            .into_iter()
            .collect(),
        )?;

        Ok(vec![positions_chunk, velocities_chunk, efforts_chunk])
    }
}
