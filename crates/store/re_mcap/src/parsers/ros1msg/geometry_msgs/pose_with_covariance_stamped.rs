use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};
use re_sdk_types::components::{RotationQuat, Translation3D};
use re_sdk_types::datatypes::Quaternion;

use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros1msg::Ros1MessageParser;
use crate::parsers::ros1msg::definitions::geometry_msgs::PoseWithCovarianceStamped;
use crate::parsers::ros1msg::wire::Ros1Reader;
use crate::util::TimestampCell;

pub struct PoseWithCovarianceStampedMessageParser {
    translations: Vec<Translation3D>,
    quaternions: Vec<RotationQuat>,
    frame_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

    use mcap::{Channel, Message};
    use re_log_types::TimeType;
    use re_sdk_types::archetypes::{CoordinateFrame, InstancePoses3D};

    use super::*;

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend((value.len() as u32).to_le_bytes());
        bytes.extend(value.as_bytes());
    }

    fn message(data: Vec<u8>) -> Message<'static> {
        Message {
            channel: Arc::new(Channel {
                id: 1,
                topic: "/pose".to_owned(),
                schema: None,
                message_encoding: "ros1".to_owned(),
                metadata: BTreeMap::default(),
            }),
            sequence: 0,
            log_time: 0,
            publish_time: 0,
            data: Cow::Owned(data),
        }
    }

    #[test]
    fn decodes_pose_with_covariance_stamped_as_instance_pose() {
        let mut data = Vec::new();
        data.extend(7_u32.to_le_bytes()); // header.seq
        data.extend(12_u32.to_le_bytes()); // stamp.sec
        data.extend(34_u32.to_le_bytes()); // stamp.nsec
        push_string(&mut data, "map");
        for value in [1.0_f64, 2.0, 3.0] {
            data.extend(value.to_le_bytes());
        }
        for value in [0.0_f64, 0.0, 0.0, 1.0] {
            data.extend(value.to_le_bytes());
        }
        for value in [0.0_f64; 36] {
            data.extend(value.to_le_bytes());
        }

        let mut parser = PoseWithCovarianceStampedMessageParser::new(1);
        let mut ctx = ParserContext::new("/pose".into(), "/pose", TimeType::TimestampNs);
        parser.append(&mut ctx, &message(data)).unwrap();

        let chunk = Box::new(parser).finalize(ctx).unwrap().remove(0);
        assert!(chunk.component_descriptors().any(|descriptor| {
            descriptor.component == InstancePoses3D::descriptor_translations().component
        }));
        assert!(chunk.component_descriptors().any(|descriptor| {
            descriptor.component == InstancePoses3D::descriptor_quaternions().component
        }));
        assert!(chunk.component_descriptors().any(|descriptor| {
            descriptor.component == CoordinateFrame::descriptor_frame().component
        }));
    }
}

impl Ros1MessageParser for PoseWithCovarianceStampedMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            translations: Vec::with_capacity(num_rows),
            quaternions: Vec::with_capacity(num_rows),
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for PoseWithCovarianceStampedMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let mut reader = Ros1Reader::new(&msg.data);
        let message = PoseWithCovarianceStamped::read(&mut reader)?;
        reader.finish()?;

        ctx.add_timestamp_cell(TimestampCell::from_nanos_ros1(
            message.header.stamp.as_nanos(),
            ctx.time_type(),
        ));

        let position = message.pose.pose.position;
        let orientation = message.pose.pose.orientation;

        self.frame_ids.push(message.header.frame_id);
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

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let pose_components: Vec<_> = InstancePoses3D::update_fields()
            .with_translations(self.translations)
            .with_quaternions(self.quaternions)
            .columns_of_unit_batches()?
            .collect();
        let frame_components: Vec<_> = CoordinateFrame::update_fields()
            .with_many_frame(self.frame_ids)
            .columns_of_unit_batches()?
            .collect();

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            ctx.entity_path().clone(),
            ctx.build_timelines(),
            pose_components
                .into_iter()
                .chain(frame_components)
                .collect(),
        )?])
    }
}
