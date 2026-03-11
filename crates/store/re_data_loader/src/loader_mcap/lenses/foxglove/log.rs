use arrow::array::StringArray;
use re_lenses::{Lens, LensError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::{MapList, Transform};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::archetypes::TextLog;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for converting [`foxglove.Log`] messages to Rerun's [`TextLog`] archetype.
///
/// [`foxglove.Log`]: https://docs.foxglove.dev/docs/sdk/schemas/log
pub fn log() -> Result<Lens, LensError> {
    Ok(
        Lens::for_input_column(EntityPathFilter::all(), "foxglove.Log:message")
            .output_columns(|out| {
                out.time(
                    FOXGLOVE_TIMESTAMP,
                    TimeType::TimestampNs,
                    Selector::parse(".timestamp")?.then(MapList::new(op::timespec_to_nanos())),
                )?
                .component(TextLog::descriptor_text(), Selector::parse(".message")?)?
                .component(
                    TextLog::descriptor_level(),
                    Selector::parse(".level.name")?.then(MapList::new(FoxgloveToRerunLogLevel)),
                )
            })?
            .build(),
    )
}

/// Maps Foxglove log level strings to Rerun [`re_sdk_types::components::TextLogLevel`] strings.
struct FoxgloveToRerunLogLevel;

impl Transform for FoxgloveToRerunLogLevel {
    type Source = StringArray;
    type Target = StringArray;

    fn transform(
        &self,
        source: &StringArray,
    ) -> Result<Option<StringArray>, re_lenses_core::combinators::Error> {
        Ok(Some(
            source
                .iter()
                .map(|level| match level {
                    Some("WARNING") => Some("WARN"),
                    Some("FATAL") => Some("CRITICAL"),
                    // Rerun has no UNKNOWN level.
                    Some("UNKNOWN") | None => None,
                    // DEBUG, INFO, ERROR can be passed through as-is.
                    other => other,
                })
                .collect(),
        ))
    }
}
