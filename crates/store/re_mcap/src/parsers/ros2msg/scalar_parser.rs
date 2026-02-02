use anyhow::Context as _;
use arrow::array::{FixedSizeListBuilder, Float64Builder};
use re_chunk::{Chunk, ChunkId, ChunkResult, EntityPath, RowId, TimePoint};
use re_sdk_types::archetypes::{Scalars, SeriesLines};

use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::ros2msg::definitions::std_msgs::Header;
use crate::parsers::util::fixed_size_list_builder;

/// Trait for extracting scalar values from ROS2 messages.
///
/// This trait allows different message types to specify which fields should be
/// extracted as scalar values for visualization in Rerun.
pub trait ScalarExtractor: serde::de::DeserializeOwned {
    /// Extract scalar values from the message.
    ///
    /// Returns a vector of (`field_name`, `value`) pairs where `field_name` is used
    /// for labeling in the visualization and `value` is the scalar measurement.
    fn extract_scalars(&self) -> Vec<(&str, f64)>;

    /// Extract the header from the message for timestamp information.
    fn header(&self) -> &Header;
}

/// Generic message parser for ROS2 messages that implement [`ScalarExtractor`].
///
/// This parser can handle any message type that implements the [`ScalarExtractor`] trait,
/// automatically extracting the specified scalar fields and logging them as [`Scalars`].
pub struct ScalarMessageParser<T: ScalarExtractor> {
    scalars: FixedSizeListBuilder<Float64Builder>,
    field_names: Vec<String>,
    num_rows: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ScalarExtractor> ScalarMessageParser<T> {
    fn init_field_names(&mut self, scalar_values: &Vec<(&str, f64)>) {
        self.field_names = scalar_values
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();

        // Recreate the builder with the correct number of fields
        if scalar_values.len() != 1 {
            // more than 2B differently named scalars? unlikely
            #[expect(clippy::cast_possible_wrap)]
            let num_scalars = scalar_values.len() as i32;
            self.scalars = fixed_size_list_builder(num_scalars, self.num_rows);
        }
    }

    /// Helper function to create a metadata chunk containing the scalar field names.
    fn metadata_chunk(entity_path: EntityPath, field_names: &[String]) -> ChunkResult<Chunk> {
        Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                TimePoint::default(), // static chunk
                &SeriesLines::new().with_names(field_names.to_vec()),
            )
            .build()
    }
}

impl<T: ScalarExtractor> Ros2MessageParser for ScalarMessageParser<T> {
    /// Create a new [`ScalarMessageParser`] for the given message type.
    fn new(num_rows: usize) -> Self {
        // We'll determine the number of fields from the first message
        Self {
            scalars: fixed_size_list_builder(1, num_rows), // Start with 1, will be recreated if needed
            field_names: Vec::new(),
            num_rows,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: ScalarExtractor> MessageParser for ScalarMessageParser<T> {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let message = cdr::try_decode_message::<T>(&msg.data).with_context(|| {
            format!(
                "Failed to decode {} message from CDR data",
                std::any::type_name::<T>()
            )
        })?;

        // Add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            message.header().stamp.as_nanos() as u64,
        ));

        let scalar_values = message.extract_scalars();

        // Initialize field names on first message
        if self.field_names.is_empty() {
            self.init_field_names(&scalar_values);
        }

        for field_name in &self.field_names {
            self.scalars.values().append_value(
                scalar_values
                    .iter()
                    .find_map(|(name, value)| (name == field_name).then_some(*value))
                    .unwrap_or(f64::NAN), // Use NaN if field not found (shouldn't happen)
            );
        }

        self.scalars.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        re_tracing::profile_function!();

        let Self {
            mut scalars,
            field_names,
            num_rows: _,
            _marker: _,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        Ok(vec![
            Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                timelines,
                std::iter::once((Scalars::descriptor_scalars(), scalars.finish().into())).collect(),
            )?,
            Self::metadata_chunk(entity_path, &field_names)?,
        ])
    }
}
