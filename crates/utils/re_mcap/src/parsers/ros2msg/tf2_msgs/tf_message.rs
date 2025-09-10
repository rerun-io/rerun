use std::collections::HashMap;

use anyhow::Context as _;
use re_chunk::{ChunkBuilder, ChunkId, EntityPath, RowId, external::nohash_hasher::IntMap};
use re_log_types::TimeCell;

use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
    ros2msg::definitions::tf2_msgs,
};

/// Parser for `tf2_msgs/msg/TFMessage` messages.
#[derive(Default)]
pub struct TfMessageParser {
    /// Store individual TF messages to process them one by one
    tf_messages: Vec<tf2_msgs::TFMessage>,
}

impl TfMessageParser {
    const ARCHETYPE_NAME: &str = "tf2_msgs.msg.TFMessage";

    pub fn new(num_rows: usize) -> Self {
        Self {
            tf_messages: Vec::with_capacity(num_rows),
        }
    }

    fn make_entity_path(parent_map: &HashMap<String, String>, child_frame: &str) -> EntityPath {
        // traverse from child to root
        let mut path = Vec::new();
        let mut current_frame = child_frame;

        loop {
            path.push(current_frame.to_owned());
            if let Some(parent) = parent_map.get(current_frame) {
                current_frame = parent;
            } else {
                break; // reached root
            }
        }

        // reverse to get path from root to child
        path.reverse();
        EntityPath::from(path.join("/"))
    }

    fn decode_all(
        tf_msg: &tf2_msgs::TFMessage,
        base_entity_path: &EntityPath,
        rerun_chunks: &mut IntMap<EntityPath, ChunkBuilder>,
    ) {
        let mut parent_map = HashMap::<String, String>::default();

        // build parent mapping (child -> parent)
        for transform in &tf_msg.transforms {
            let parent_frame = transform.header.frame_id.clone();
            let child_frame = transform.child_frame_id.clone();
            parent_map.insert(child_frame, parent_frame);
        }

        for transform in &tf_msg.transforms {
            let child_frame = &transform.child_frame_id;
            let relative_path = Self::make_entity_path(&parent_map, child_frame);
            // Combine the base entity path with the TF frame hierarchy
            let entity_path = base_entity_path.join(&relative_path);

            // Build TimePoint from the sensor timestamp in the transform header
            let timepoint = re_chunk::TimePoint::default().with_index(
                "timestamp",
                TimeCell::from_timestamp_nanos_since_epoch(transform.header.stamp.as_nanos()),
            );

            let (_, chunk_builder) = rerun_chunks.remove_entry(&entity_path).unwrap_or_else(|| {
                (
                    entity_path.clone(),
                    ChunkBuilder::new(ChunkId::new(), entity_path.clone()),
                )
            });

            let chunk_builder = chunk_builder
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::Transform3D::from_translation((
                        transform.transform.translation.x as f32,
                        transform.transform.translation.y as f32,
                        transform.transform.translation.z as f32,
                    ))
                    .with_quaternion(re_types::datatypes::Quaternion::from_xyzw([
                        transform.transform.rotation.x as f32,
                        transform.transform.rotation.y as f32,
                        transform.transform.rotation.z as f32,
                        transform.transform.rotation.w as f32,
                    ]))
                    .with_axis_length(0.15),
                )
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::Points3D::new([(0.0, 0.0, 0.0)])
                        .with_labels([child_frame.clone()]),
                );

            rerun_chunks.insert(entity_path.clone(), chunk_builder);
        }
    }
}

impl MessageParser for TfMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let tf_msg = cdr::try_decode_message::<tf2_msgs::TFMessage>(&msg.data)
            .context("Failed to decode tf2_msgs::TFMessage message from CDR data")?;

        // Add timestamps from the first transform in the message (if any) to the context
        if let Some(first_transform) = tf_msg.transforms.first() {
            ctx.add_time_cell(
                "timestamp",
                TimeCell::from_timestamp_nanos_since_epoch(first_transform.header.stamp.as_nanos()),
            );
        }

        self.tf_messages.push(tf_msg);
        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<re_chunk::Chunk>> {
        re_tracing::profile_function!();
        let Self { tf_messages } = *self;

        let base_entity_path = ctx.entity_path().clone();
        let mut rerun_chunks: IntMap<EntityPath, ChunkBuilder> = IntMap::default();

        for tf_msg in &tf_messages {
            Self::decode_all(tf_msg, &base_entity_path, &mut rerun_chunks);
        }

        let chunks = rerun_chunks
            .into_values()
            .map(|chunk_builder| chunk_builder.build())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(chunks)
    }
}
