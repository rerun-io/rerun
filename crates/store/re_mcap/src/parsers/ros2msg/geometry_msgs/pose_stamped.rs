use super::super::definitions::geometry_msgs::PoseStamped;
use re_chunk::{Chunk, ChunkId};

// TODO: archetype is just an example to visualize _something_. Transform3D would be not entirely correct here,
// as a pose is not a frame-to-frame relationship (pose vs transform is similar to point vs vector). tbd!
use re_types::components::Vector3D;
use re_types::{archetypes::Arrows3D, components::Position3D};

use super::super::Ros2MessageParser;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
};
use crate::util::TimestampCell;

use glam::{DQuat, DVec3};

pub struct PoseStampedMessageParser {
    // Here we just use the oriented x vector of the pose, mimicking the RViz posestamped display
    // TODO: use sth better than arrows
    x_vectors: Vec<Vector3D>,
    origins: Vec<Position3D>,
}

impl Ros2MessageParser for PoseStampedMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            x_vectors: Vec::with_capacity(num_rows),
            origins: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for PoseStampedMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let PoseStamped { header, pose } = cdr::try_decode_message::<PoseStamped>(&msg.data)?;

        // TODO: use frame_id

        // Add the header timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));
        let position = Position3D::new(
            pose.position.x as f32,
            pose.position.y as f32,
            pose.position.z as f32,
        );
        // TODO: normalize or error if not normalized?
        let orientation = DQuat::from_xyzw(
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        )
        .normalize();

        let direction_x = orientation * DVec3::X;
        self.x_vectors.push(Vector3D::from([
            direction_x.x as f32,
            direction_x.y as f32,
            direction_x.z as f32,
        ]));

        self.origins.push(position);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self { x_vectors, origins } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let components = Arrows3D::update_fields()
            .with_vectors(x_vectors)
            .with_origins(origins)
            .columns_of_unit_batches()?
            .collect();

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            components,
        )?;

        Ok(vec![chunk])
    }
}
