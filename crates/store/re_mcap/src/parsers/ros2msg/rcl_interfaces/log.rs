use anyhow::Context as _;
use arrow::array::{FixedSizeListArray, FixedSizeListBuilder, StringBuilder, UInt32Builder};
use re_chunk::{Chunk, ChunkComponents, ChunkId};
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::{Color, Text, TextLogLevel};
use re_sdk_types::datatypes::Rgba32;
use re_sdk_types::{ComponentDescriptor, SerializedComponentColumn};

use crate::parsers::cdr;
use crate::parsers::decode::{MessageParser, ParserContext};
use crate::parsers::ros2msg::Ros2MessageParser;
use crate::parsers::ros2msg::definitions::rcl_interfaces::{self, LogLevel};
use crate::parsers::util::fixed_size_list_builder;

pub struct LogMessageParser {
    text_entries: Vec<String>,
    levels: Vec<String>,
    colors: Vec<Color>,
    file: FixedSizeListBuilder<StringBuilder>,
    function: FixedSizeListBuilder<StringBuilder>,
    line: FixedSizeListBuilder<UInt32Builder>,
}

impl LogMessageParser {
    const ARCHETYPE_NAME: &str = "rcl_interfaces.msg.Log";

    fn create_metadata_column(name: &str, array: FixedSizeListArray) -> SerializedComponentColumn {
        SerializedComponentColumn {
            list_array: array.into(),
            descriptor: ComponentDescriptor::partial(name)
                .with_archetype(Self::ARCHETYPE_NAME.into()),
        }
    }

    fn ros2_level_to_color(level: LogLevel) -> Color {
        match level {
            LogLevel::Info => Color::from(Rgba32::from_rgb(0, 128, 255)), // Blue
            LogLevel::Warn => Color::from(Rgba32::from_rgb(255, 165, 0)), // Orange
            LogLevel::Error => Color::from(Rgba32::from_rgb(255, 0, 0)),  // Red
            LogLevel::Fatal => Color::from(Rgba32::from_rgb(139, 0, 0)),  // Dark Red
            LogLevel::Unknown | LogLevel::Debug => {
                Color::from(Rgba32::from_rgb(128, 128, 128)) // Gray
            }
        }
    }
}

impl Ros2MessageParser for LogMessageParser {
    fn new(num_rows: usize) -> Self {
        Self {
            text_entries: Vec::with_capacity(num_rows),
            levels: Vec::with_capacity(num_rows),
            colors: Vec::with_capacity(num_rows),
            file: fixed_size_list_builder(1, num_rows),
            function: fixed_size_list_builder(1, num_rows),
            line: fixed_size_list_builder(1, num_rows),
        }
    }
}

impl MessageParser for LogMessageParser {
    fn append(&mut self, ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let rcl_interfaces::Log {
            stamp,
            level,
            name,
            msg: log_msg,
            file,
            function,
            line,
        } = cdr::try_decode_message::<rcl_interfaces::Log>(&msg.data)
            .context("Failed to decode `rcl_interfaces::Log` message from CDR data")?;

        // add the sensor timestamp to the context, `log_time` and `publish_time` are added automatically
        ctx.add_timestamp_cell(crate::util::TimestampCell::guess_from_nanos_ros2(
            stamp.as_nanos() as u64,
        ));

        self.text_entries.push(format!("[{name}] {log_msg}"));
        self.levels.push(level.to_string());
        self.colors.push(Self::ros2_level_to_color(level));

        self.file.values().append_value(file);
        self.file.append(true);

        self.function.values().append_value(function);
        self.function.append(true);

        self.line.values().append_slice(&[line]);
        self.line.append(true);

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        re_tracing::profile_function!();
        let Self {
            text_entries,
            levels,
            colors,
            mut file,
            mut function,
            mut line,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        let text_components: Vec<Text> = text_entries.into_iter().map(Text::from).collect();
        let level_components: Vec<TextLogLevel> =
            levels.into_iter().map(TextLogLevel::from).collect();

        let text_log = TextLog::update_fields()
            .with_many_text(text_components)
            .with_many_level(level_components)
            .with_many_color(colors);

        let mut chunk_components: Vec<SerializedComponentColumn> =
            text_log.columns_of_unit_batches()?.collect();

        // TODO(#11098): these should be part of the `TextLog` archetype instead
        chunk_components.extend([
            Self::create_metadata_column("file", file.finish()),
            Self::create_metadata_column("function", function.finish()),
            Self::create_metadata_column("line", line.finish()),
        ]);

        let components: ChunkComponents = chunk_components.into_iter().collect();

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            components,
        )?])
    }
}
