use re_lenses::{LensBuilderError, Lenses};
use re_log_types::TimeType;

/// Adds all Foxglove lenses to an existing collection.
pub fn add_foxglove_lenses(
    lenses: Lenses,
    time_type: TimeType,
) -> Result<Lenses, LensBuilderError> {
    Ok(re_lenses::semantic::foxglove::all(time_type)?
        .into_iter()
        .fold(lenses, Lenses::add_lens))
}
