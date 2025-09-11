use super::super::definitions::sensor_msgs;
use arrow::array::{FixedSizeListBuilder, Float64Builder};

use re_chunk::{Chunk, ChunkId};
use re_log_types::TimeCell;
use re_types::{
    ComponentDescriptor, SerializedComponentColumn, archetypes::Arrows3D, datatypes::Vec3D,
};

use crate::{
    Error,
    parsers::{MessageParser, ParserContext, cdr, util::fixed_size_list_builder},
};

/// Plugin that parses `sensor_msgs/msg/MagneticField` messages.
#[derive(Default)]
pub struct MagneticFieldSchemaPlugin;

pub struct MagneticFieldMessageParser {
    vectors: Vec<Vec3D>,
    magnetic_field_covariance: FixedSizeListBuilder<Float64Builder>,
}

impl MagneticFieldMessageParser {
    const ARCHETYPE_NAME: &str = "sensor_msgs.msg.MagneticField";

    /// Create a new [`MagneticFieldMessageParser`]
    pub fn new(num_rows: usize) -> Self {
        Self {
            vectors: Vec::with_capacity(num_rows),
            magnetic_field_covariance: fixed_size_list_builder(9, num_rows),
        }
    }
}

impl MessageParser for MagneticFieldMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        let magnetic_field =
            cdr::try_decode_message::<sensor_msgs::MagneticField>(msg.data.as_ref())
                .map_err(|err| Error::Other(anyhow::anyhow!(err)))?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            crate::util::guess_epoch(magnetic_field.header.stamp.as_nanos() as u64),
        );

        // Convert magnetic field vector to Vector3D and store
        self.vectors.push(Vec3D([
            magnetic_field.magnetic_field.x as f32,
            magnetic_field.magnetic_field.y as f32,
            magnetic_field.magnetic_field.z as f32,
        ]));

        // Store covariance
        self.magnetic_field_covariance
            .values()
            .append_slice(&magnetic_field.magnetic_field_covariance);
        self.magnetic_field_covariance.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let Self {
            vectors,
            mut magnetic_field_covariance,
        } = *self;

        let mut chunk_components: Vec<_> = Arrows3D::update_fields()
            .with_vectors(vectors)
            .columns_of_unit_batches()?
            .collect();

        chunk_components.push(SerializedComponentColumn {
            descriptor: ComponentDescriptor::partial("magnetic_field_covariance")
                .with_archetype(Self::ARCHETYPE_NAME.into()),
            list_array: magnetic_field_covariance.finish().into(),
        });

        let data_chunk = Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path.clone(),
            timelines,
            chunk_components.into_iter().collect(),
        )?;

        Ok(vec![data_chunk])
    }
}
