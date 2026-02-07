use re_chunk::{Chunk, ChunkId};
use re_sdk_types::archetypes::{Arrows3D, CoordinateFrame};
use re_sdk_types::datatypes::Vec3D;

use super::super::definitions::sensor_msgs;
use crate::Error;
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::{MessageParser, ParserContext, cdr};

pub struct MagneticFieldMessageParser {
    vectors: Vec<Vec3D>,
    frame_ids: Vec<String>,
}

impl Ros2MessageParser for MagneticFieldMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            vectors: Vec::with_capacity(num_rows),
            frame_ids: Vec::with_capacity(num_rows),
        }
    }
}

impl MessageParser for MagneticFieldMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let magnetic_field =
            cdr::try_decode_message::<sensor_msgs::MagneticField>(msg.data.as_ref())
                .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            magnetic_field.header.stamp.as_nanos() as u64,
        ));

        self.frame_ids.push(magnetic_field.header.frame_id);

        // Convert magnetic field vector to Vector3D and store
        self.vectors.push(Vec3D([
            magnetic_field.magnetic_field.x as f32,
            magnetic_field.magnetic_field.y as f32,
            magnetic_field.magnetic_field.z as f32,
        ]));

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self { vectors, frame_ids } = *self;

        let mut components: Vec<_> = Arrows3D::update_fields()
            .with_vectors(vectors)
            .columns_of_unit_batches()?
            .collect();

        components.extend(
            CoordinateFrame::update_fields()
                .with_many_frame(frame_ids)
                .columns_of_unit_batches()?,
        );

        let data_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            components.into_iter().collect(),
        )?;

        Ok(vec![data_chunk])
    }
}
