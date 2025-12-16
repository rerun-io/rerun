use super::super::definitions::tf2_msgs::TFMessage;
use re_chunk::{Chunk, ChunkId};

use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::components::{RotationQuat, Translation3D};
use re_sdk_types::datatypes::Quaternion;

use super::super::Ros2MessageParser;
use crate::parsers::ros2msg::definitions::geometry_msgs::{Transform, TransformStamped};
use crate::parsers::ros2msg::definitions::std_msgs::Header;
use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
};
use crate::util::{TimestampCell, log_and_publish_timepoint_from_msg};

pub struct TfMessageParser {
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    parent_frame_ids: Vec<String>,
    child_frame_ids: Vec<String>,
}

impl Ros2MessageParser for TfMessageParser {
    fn new(_num_rows: usize) -> Self {
        // Note that we can't know the number of output rows in advance,
        // as each message can contain a variable amount of transforms.
        Self {
            translations: Vec::new(),
            quaternions: Vec::new(),
            parent_frame_ids: Vec::new(),
            child_frame_ids: Vec::new(),
        }
    }
}

impl MessageParser for TfMessageParser {
    fn get_log_and_publish_timepoints(
        &self,
        msg: &mcap::Message<'_>,
    ) -> anyhow::Result<Vec<re_chunk::TimePoint>> {
        // We need a custom implementation of this method because we have a 1-to-N relationship between input messages and output rows.
        // Assign each output row the same log and publish time as the input message.
        let TFMessage { transforms } = cdr::try_decode_message::<TFMessage>(&msg.data)?;
        Ok(vec![
            log_and_publish_timepoint_from_msg(msg);
            transforms.len()
        ])
    }

    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let TFMessage { transforms } = cdr::try_decode_message::<TFMessage>(&msg.data)?;

        // Each transform in the message has its own timestamp.
        for TransformStamped {
            header,
            child_frame_id,
            transform,
        } in transforms
        {
            // Add the header timestamp to the context.
            // `log_time` and `publish_time` are added via `log_and_publish_time_from_msg`.
            let Header { stamp, frame_id } = header;
            ctx.add_timestamp_cell(TimestampCell::guess_from_nanos_ros2(stamp.as_nanos() as u64));

            self.parent_frame_ids.push(frame_id);
            self.child_frame_ids.push(child_frame_id);

            let Transform {
                translation,
                rotation,
            } = transform;
            self.translations.push(Translation3D::new(
                translation.x as f32,
                translation.y as f32,
                translation.z as f32,
            ));
            self.quaternions.push(
                Quaternion::from_xyzw([
                    rotation.x as f32,
                    rotation.y as f32,
                    rotation.z as f32,
                    rotation.w as f32,
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

        let chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines.clone(),
            Transform3D::update_fields()
                .with_many_translation(translations)
                .with_many_quaternion(quaternions)
                .with_many_child_frame(child_frame_ids)
                .with_many_parent_frame(parent_frame_ids)
                .columns_of_unit_batches()?
                .collect(),
        )?;

        Ok(vec![chunk])
    }
}
