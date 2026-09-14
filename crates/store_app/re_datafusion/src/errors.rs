use datafusion::error::DataFusionError;
use re_redap_client::ApiError;

/// Extension trait for converting an [`ApiError`] into a [`DataFusionError::External`].
///
/// In general, we want [`DataFusionError::External`] to always wrap an [`ApiError`],
/// and never anything else.
pub(crate) trait IntoDfError {
    fn into_df_error(self) -> DataFusionError;
}

impl IntoDfError for ApiError {
    fn into_df_error(self) -> DataFusionError {
        DataFusionError::External(Box::new(self))
    }
}
