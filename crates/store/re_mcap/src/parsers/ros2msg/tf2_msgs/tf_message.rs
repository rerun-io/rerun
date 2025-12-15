use super::super::definitions::tf2_msgs::TFMessage;
use re_chunk::{Chunk, ChunkId};

use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::components::{RotationQuat, Translation3D};
use re_sdk_types::datatypes::Quaternion;

use super::super::Ros2MessageParser;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
};
use crate::util::TimestampCell;

pub struct TfMessageMessageParser {
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    parent_frame_ids: Vec<String>,
    child_frame_ids: Vec<String>,
}

impl Ros2MessageParser for TfMessageMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            translations: Vec::with_capacity(num_rows),
            quaternions: Vec::with_capacity(num_rows),
            parent_frame_ids: Vec::with_capacity(num_rows),
            child_frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for TfMessageMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let TFMessage { transforms } = cdr::try_decode_message::<TFMessage>(&msg.data)?;

        // Each transform in the message has its own timestamp.
        for transform in transforms {
            // Add the header timestamp to the context, `log_time` and `publish_time` are added automatically
            ctx.add_timestamp_cell(TimestampCell::guess_from_nanos_ros2(
                transform.header.stamp.as_nanos() as u64,
            ));

            self.parent_frame_ids.push(transform.header.frame_id);
            self.child_frame_ids.push(transform.child_frame_id);

            self.translations.push(Translation3D::new(
                transform.transform.translation.x as f32,
                transform.transform.translation.y as f32,
                transform.transform.translation.z as f32,
            ));
            self.quaternions.push(
                Quaternion::from_xyzw([
                    transform.transform.rotation.x as f32,
                    transform.transform.rotation.y as f32,
                    transform.transform.rotation.z as f32,
                    transform.transform.rotation.w as f32,
                ])
                .into(),
            );
        }

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self {
            translations,
            quaternions,
            parent_frame_ids,
            child_frame_ids,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let pose_components: Vec<_> = Transform3D::update_fields()
            .with_many_translation(translations)
            .with_many_quaternion(quaternions)
            .with_many_child_frame(child_frame_ids)
            .with_many_parent_frame(parent_frame_ids)
            .columns_of_unit_batches()?
            .collect();

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            pose_components.into_iter().collect(),
        )?;

        Ok(vec![chunk])
    }
}
