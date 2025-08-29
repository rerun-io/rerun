use re_chunk::{Chunk, ChunkId};
use re_types::{
    archetypes::TextLog,
    components::{Color, Text, TextLogLevel},
    datatypes::Rgba32,
};

use crate::parsers::{
    cdr,
    decode::{MessageParser, ParserContext},
    ros2msg::definitions::rcl_interfaces,
};

/// Plugin that parses `rcl_interfaces/msg/Log` messages.
#[derive(Default)]
pub struct LogSchemaPlugin;

pub struct LogMessageParser {
    text_entries: Vec<String>,
    levels: Vec<String>,
    colors: Vec<Option<Color>>,
}

impl LogMessageParser {
    pub fn new(num_rows: usize) -> Self {
        Self {
            text_entries: Vec::with_capacity(num_rows),
            levels: Vec::with_capacity(num_rows),
            colors: Vec::with_capacity(num_rows),
        }
    }

    fn ros2_level_to_rerun_level(level: rcl_interfaces::LogLevel) -> &'static str {
        match level {
            rcl_interfaces::LogLevel::Debug => "DEBUG",
            rcl_interfaces::LogLevel::Info => "INFO",
            rcl_interfaces::LogLevel::Warn => "WARN",
            rcl_interfaces::LogLevel::Error => "ERROR",
            rcl_interfaces::LogLevel::Fatal => "CRITICAL",
            rcl_interfaces::LogLevel::Unknown => "TRACE",
        }
    }

    fn ros2_level_to_color(level: rcl_interfaces::LogLevel) -> Option<Color> {
        match level {
            rcl_interfaces::LogLevel::Debug => Some(Color::from(Rgba32::from_rgb(128, 128, 128))), // Gray
            rcl_interfaces::LogLevel::Info => Some(Color::from(Rgba32::from_rgb(0, 128, 255))), // Blue
            rcl_interfaces::LogLevel::Warn => Some(Color::from(Rgba32::from_rgb(255, 165, 0))), // Orange
            rcl_interfaces::LogLevel::Error => Some(Color::from(Rgba32::from_rgb(255, 0, 0))), // Red
            rcl_interfaces::LogLevel::Fatal => Some(Color::from(Rgba32::from_rgb(139, 0, 0))), // Dark Red
            rcl_interfaces::LogLevel::Unknown => None,
        }
    }
}

impl MessageParser for LogMessageParser {
    fn append(&mut self, _ctx: &mut ParserContext, msg: &mcap::Message<'_>) -> anyhow::Result<()> {
        re_tracing::profile_function!();
        let rcl_interfaces::Log {
            stamp: _stamp,
            level,
            name,
            msg: log_msg,
            file,
            function,
            line: _line,
        } = match cdr::try_decode_message::<rcl_interfaces::Log>(&msg.data) {
            Ok(log) => log,
            Err(e) => {
                // Log detailed diagnostic information about the decode failure
                re_log::warn!(
                    "Failed to decode rcl_interfaces::Log message from CDR data. Data length: {}, first 16 bytes: {:02x?}, error: {}",
                    msg.data.len(),
                    &msg.data.get(..16.min(msg.data.len())).unwrap_or(&[]),
                    e
                );

                // Return early to skip this message but continue processing others
                return Ok(());
            }
        };
        // Format the log message with additional context information
        let formatted_msg = if !name.is_empty() || !file.is_empty() || !function.is_empty() {
            format!(
                "[{}] {}",
                if name.is_empty() { String::new() } else { name },
                log_msg
            )
        } else {
            log_msg
        };

        self.text_entries.push(formatted_msg);
        self.levels
            .push(Self::ros2_level_to_rerun_level(level).to_owned());
        self.colors.push(Self::ros2_level_to_color(level));

        Ok(())
    }

    fn finalize(self: Box<Self>, ctx: ParserContext) -> anyhow::Result<Vec<Chunk>> {
        re_tracing::profile_function!();
        let Self {
            text_entries,
            levels,
            colors,
        } = *self;

        let entity_path = ctx.entity_path().clone();
        let timelines = ctx.build_timelines();

        // Convert to the component types
        let text_components: Vec<Text> = text_entries.into_iter().map(Text::from).collect();
        let level_components: Vec<TextLogLevel> =
            levels.into_iter().map(TextLogLevel::from).collect();

        let mut text_log = TextLog::update_fields()
            .with_many_text(text_components)
            .with_many_level(level_components);

        // Add colors if any are present
        let filtered_colors: Vec<Color> = colors.into_iter().flatten().collect();
        if !filtered_colors.is_empty() {
            text_log = text_log.with_many_color(filtered_colors);
        }

        let chunk_components = text_log.columns_of_unit_batches()?.collect();

        Ok(vec![Chunk::from_auto_row_ids(
            ChunkId::new(),
            entity_path,
            timelines,
            chunk_components,
        )?])
    }
}
