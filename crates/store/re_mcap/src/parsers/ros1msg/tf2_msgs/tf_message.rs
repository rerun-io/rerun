use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::components::{RotationQuat, Translation3D};
use re_sdk_types::datatypes::Quaternion;

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::geometry_msgs::{Transform, TransformStamped};
use crate::parsers::ros1msg::definitions::std_msgs::Header;
use crate::parsers::ros1msg::definitions::tf2_msgs::TFMessage;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::util::{TimestampCell, log_and_publish_timepoint_from_msg};

const STATIC_TF_TOPIC: &str = "/tf_static";

fn static_chunk_timelines()
-> re_chunk::external::nohash_hasher::IntMap<re_log_types::TimelineName, re_chunk::TimeColumn> {
    re_chunk::external::nohash_hasher::IntMap::default()
}

fn decode_tf_message(data: &[u8]) -> anyhow::Result<TFMessage> {
    let mut reader = Ros1Reader::new(data);
    let message = TFMessage::read(&mut reader)?;
    reader.finish()?;
    Ok(message)
}

pub struct TfMessageParser {
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    parent_frame_ids: Vec<String>,
    child_frame_ids: Vec<String>,
}

impl Ros1MessageParser for TfMessageParser {
    fn new(_num_rows: usize) -> Self {
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
        time_type: re_log_types::TimeType,
    ) -> anyhow::Result<Vec<re_chunk::TimePoint>> {
        Ok(vec![
            log_and_publish_timepoint_from_msg(msg, time_type);
            decode_tf_message(&msg.data)?.transforms.len()
        ])
    }

    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        for TransformStamped {
            header: Header {
                stamp, frame_id, ..
            },
            child_frame_id,
            transform:
                Transform {
                    translation,
                    rotation,
                },
        } in decode_tf_message(&msg.data)?.transforms
        {
            ctx.add_timestamp_cell(TimestampCell::from_nanos_ros1(
                stamp.as_nanos(),
                ctx.time_type(),
            ));
            self.parent_frame_ids.push(frame_id);
            self.child_frame_ids.push(child_frame_id);
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

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = if ctx.channel_topic() == STATIC_TF_TOPIC {
            static_chunk_timelines()
        } else {
            ctx.build_timelines()
        };

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            Transform3D::update_fields()
                .with_many_translation(self.translations)
                .with_many_quaternion(self.quaternions)
                .with_many_child_frame(self.child_frame_ids)
                .with_many_parent_frame(self.parent_frame_ids)
                .columns_of_unit_batches()?
                .collect(),
        )?])
    }
}
