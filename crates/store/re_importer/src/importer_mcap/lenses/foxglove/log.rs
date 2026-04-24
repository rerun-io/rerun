use arrow::array::{Array as _, ArrayRef, StringArray};
use re_lenses::{Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::Error;
use re_log_types::TimeType;
use re_sdk_types::archetypes::TextLog;

use super::FOXGLOVE_TIMESTAMP;

/// Creates a lens for converting [`foxglove.Log`] messages to Rerun's [`TextLog`] archetype.
///
/// [`foxglove.Log`]: https://docs.foxglove.dev/docs/sdk/schemas/log
pub fn log(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    Ok(Lens::for_input_column("foxglove.Log:message")
        .output_columns(|out| {
            out.time(
                FOXGLOVE_TIMESTAMP,
                time_type,
                Selector::parse(".timestamp")?.pipe(op::timespec_to_nanos()),
            )?
            .component(TextLog::descriptor_text(), Selector::parse(".message")?)?
            .component(
                TextLog::descriptor_level(),
                Selector::parse(".level.name")?.pipe(foxglove_to_rerun_log_level()),
            )
        })?
        .build())
}

/// Returns a pipe-compatible function that maps Foxglove log level strings to Rerun
/// [`re_sdk_types::components::TextLogLevel`] strings.
fn foxglove_to_rerun_log_level() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> {
    move |source: &ArrayRef| {
        let source = source
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StringArray".to_owned(),
                actual: source.data_type().clone(),
                context: "foxglove_to_rerun_log_level input".to_owned(),
            })?;

        let result: StringArray = source
            .iter()
            .map(|level| match level {
                Some("WARNING") => Some("WARN"),
                Some("FATAL") => Some("CRITICAL"),
                // Rerun has no UNKNOWN level.
                Some("UNKNOWN") | None => None,
                // DEBUG, INFO, ERROR can be passed through as-is.
                other => other,
            })
            .collect();

        Ok(Some(std::sync::Arc::new(result) as ArrayRef))
    }
}
