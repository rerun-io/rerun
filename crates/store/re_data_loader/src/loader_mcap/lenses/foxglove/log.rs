use arrow::array::{ListArray, StringArray};
use re_arrow_combinators::{Transform, map::MapList};
use re_lenses::{Lens, LensError, Op, OpError};
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
                    [Op::selector(".timestamp"), Op::time_spec_to_nanos()],
                )
                .component(TextLog::descriptor_text(), [Op::selector(".message")])
                .component(
                    TextLog::descriptor_level(),
                    [
                        Op::selector(".level.name"),
                        Op::func(foxglove_level_to_rerun),
                    ],
                )
            })?
            .build(),
    )
}

/// Maps Foxglove log level names to Rerun [`re_sdk_types::components::TextLogLevel`] names.
fn foxglove_level_to_rerun(list_array: &ListArray) -> Result<ListArray, OpError> {
    Ok(MapList::new(FoxgloveToRerunLogLevel).transform(list_array)?)
}

/// Maps Foxglove log level strings to Rerun [`re_sdk_types::components::TextLogLevel`] strings.
struct FoxgloveToRerunLogLevel;

impl Transform for FoxgloveToRerunLogLevel {
    type Source = StringArray;
    type Target = StringArray;

    fn transform(&self, source: &StringArray) -> Result<StringArray, re_arrow_combinators::Error> {
        Ok(source
            .iter()
            .map(|level| match level {
                Some("WARNING") => Some("WARN"),
                Some("FATAL") => Some("CRITICAL"),
                // Rerun has no UNKNOWN level.
                Some("UNKNOWN") | None => None,
                // DEBUG, INFO, ERROR can be passed through as-is.
                other => other,
            })
            .collect())
    }
}
