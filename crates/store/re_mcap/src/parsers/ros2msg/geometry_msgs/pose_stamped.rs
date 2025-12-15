use super::super::definitions::geometry_msgs::{Pose, PoseStamped};
use re_chunk::{Chunk, ChunkId};

use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};
use re_sdk_types::components::{RotationQuat, Translation3D};
use re_sdk_types::datatypes::Quaternion;

use super::super::Ros2MessageParser;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
};
use crate::util::TimestampCell;

pub struct PoseStampedMessageParser {
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    frame_ids: Vec<String>,
}

impl Ros2MessageParser for PoseStampedMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            translations: Vec::with_capacity(num_rows),
            quaternions: Vec::with_capacity(num_rows),
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for PoseStampedMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let PoseStamped { header, pose } = cdr::try_decode_message::<PoseStamped>(&msg.data)?;
        let Pose {
            position,
            orientation,
        } = pose;

        // Add the header timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(TimestampCell::guess_from_nanos_ros2(
            header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.push(header.frame_id);
        self.translations.push(Translation3D::new(
            position.x as f32,
            position.y as f32,
            position.z as f32,
        ));
        self.quaternions.push(
            Quaternion::from_xyzw([
                orientation.x as f32,
                orientation.y as f32,
                orientation.z as f32,
                orientation.w as f32,
            ])
            .into(),
        );

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            translations,
            quaternions,
            frame_ids: frames,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let pose_components: Vec<_> = InstancePoses3D::update_fields()
            .with_translations(translations)
            .with_quaternions(quaternions)
            .columns_of_unit_batches()?
            .collect();
        let frame_components: Vec<_> = CoordinateFrame::update_fields()
            .with_many_frame(frames)
            .columns_of_unit_batches()?
            .collect();

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            pose_components
                .into_iter()
                .chain(frame_components.into_iter())
                .collect(),
        )?;

        Ok(vec![chunk])
    }
}
