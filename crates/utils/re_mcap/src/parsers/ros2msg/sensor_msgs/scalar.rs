use anyhow::Context as _;
use arrow::array::{FixedSizeListBuilder, Float64Builder};
use re_chunk::{Chunk, ChunkId, ChunkResult, EntityPath, RowId, TimePoint};
use re_log_types::TimeCell;
use re_types::archetypes::{Scalars, SeriesLines};

use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
    ros2msg::definitions::{
        sensor_msgs::{BatteryState, FluidPressure, Range, Temperature},
        std_msgs::Header,
    },
    util::fixed_size_list_builder,
};

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

    /// Returns the archetype name for this message type.
    fn archetype_name() -> &'static str;
}

/// Generic message parser for ROS2 messages that implement [`ScalarExtractor`].
///
/// This parser can handle any message type that implements the [`ScalarExtractor`] trait,
/// automatically extracting the specified scalar fields and logging them as Rerun Scalars.
pub struct ScalarMessageParser<T: ScalarExtractor> {
    scalars: FixedSizeListBuilder<Float64Builder>,
    field_names: Vec<String>,
    num_rows: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: ScalarExtractor> ScalarMessageParser<T> {
    /// Create a new [`ScalarMessageParser`] for the given message type.
    pub fn new(num_rows: usize) -> Self {
        // We'll determine the number of fields from the first message
        Self {
            scalars: fixed_size_list_builder(1, num_rows), // Start with 1, will be recreated if needed
            field_names: Vec::new(),
            num_rows,
            _marker: std::marker::PhantomData,
        }
    }

    fn init_field_names(&mut self, scalar_values: &Vec<(&str, f64)>) {
        self.field_names = scalar_values
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();

        // Recreate the builder with the correct number of fields
        if scalar_values.len() != 1 {
            self.scalars = fixed_size_list_builder(scalar_values.len() as i32, self.num_rows);
        }
    }

    /// Helper function to create a metadata chunk containing the scalar field names.
    fn metadata_chunk(entity_path: EntityPath, field_names: &[String]) -> ChunkResult<Chunk> {
        Chunk::builder(entity_path)
            .with_archetype(
                RowId::new(),
                TimePoint::default(),
                &SeriesLines::new().with_names(field_names.to_vec()),
            )
            .build()
    }
}

impl<T: ScalarExtractor> MessageParser for ScalarMessageParser<T> {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();

        let message = cdr::try_decode_message::<T>(&msg.data).with_context(|| {
            format!(
                "Failed to decode {} message from CDR data",
                T::archetype_name()
            )
        })?;

        // Add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_time_cell(
            "timestamp",
            TimeCell::from_timestamp_nanos_since_epoch(message.header().stamp.as_nanos()),
        );

        let scalar_values = message.extract_scalars();

        // Initialize field names on first message
        if self.field_names.is_empty() {
            self.init_field_names(&scalar_values);
        }

        let values: Vec<f64> = self
            .field_names
            .iter()
            .map(|field_name| {
                scalar_values
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, value)| *value)
                    .unwrap_or(f64::NAN) // Use NaN if field not found (shouldn't happen)
            })
            .collect();

        self.scalars.values().append_slice(&values);
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

// Implement ScalarExtractor for each message type

impl ScalarExtractor for Temperature {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("temperature", self.temperature),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }

    fn archetype_name() -> &'static str {
        "sensor_msgs.msg.Temperature"
    }
}

impl ScalarExtractor for FluidPressure {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("fluid_pressure", self.fluid_pressure),
            ("variance", self.variance),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }

    fn archetype_name() -> &'static str {
        "sensor_msgs.msg.FluidPressure"
    }
}

impl ScalarExtractor for Range {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("range", self.range as f64),
            ("min_range", self.min_range as f64),
            ("max_range", self.max_range as f64),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }

    fn archetype_name() -> &'static str {
        "sensor_msgs.msg.Range"
    }
}

impl ScalarExtractor for BatteryState {
    fn extract_scalars(&self) -> Vec<(&str, f64)> {
        vec![
            ("percentage", self.percentage as f64),
            ("voltage", self.voltage as f64),
            ("current", self.current as f64),
            ("charge", self.charge as f64),
            ("temperature", self.temperature as f64),
        ]
    }

    fn header(&self) -> &Header {
        &self.header
    }

    fn archetype_name() -> &'static str {
        "sensor_msgs.msg.BatteryState"
    }
}

// Type aliases for convenience
pub type TemperatureMessageParser = ScalarMessageParser<Temperature>;
pub type FluidPressureMessageParser = ScalarMessageParser<FluidPressure>;
pub type RangeMessageParser = ScalarMessageParser<Range>;
pub type BatteryStateMessageParser = ScalarMessageParser<BatteryState>;
