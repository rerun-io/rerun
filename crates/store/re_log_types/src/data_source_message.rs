// TODO(andreas): Conceptually these should go to `re_data_source`.
// However, `re_data_source` depends on everything that _implements_ a datasource, therefore we would get a circular dependency!

use crate::{AbsoluteTimeRange, LogMsg, StoreId, TimelineName, impl_into_enum};

/// Message from a data source.
///
/// May contain limited UI commands for instrumenting the state of the receiving end.
#[derive(Clone, Debug)]
pub enum DataSourceMessage {
    /// See [`LogMsg`].
    LogMsg(LogMsg),

    /// A UI command that has to be ordered relative to [`LogMsg`]s.
    ///
    /// Non-ui receivers can safely ignore these.
    UiCommand(DataSourceUiCommand),
}

impl_into_enum!(LogMsg, DataSourceMessage, LogMsg);
impl_into_enum!(DataSourceUiCommand, DataSourceMessage, UiCommand);

/// UI commands issued when streaming in datasets.
///
/// If you're not in a ui context you can safely ignore these.
#[derive(Clone, Debug)]
pub enum DataSourceUiCommand {
    /// Mark a time range as valid.
    ///
    /// Everything outside can still be navigated to, but will be considered potentially lacking some data and therefore "invalid".
    /// Visually, it is outside of the normal time range and shown greyed out.
    ///
    /// If timeline is `None`, this signals that all timelines are considered to be valid entirely.
    AddValidTimeRange {
        store_id: StoreId,

        /// If `None`, signals that all timelines are entirely valid.
        timeline: Option<TimelineName>,
        time_range: AbsoluteTimeRange,
    },

    /// Navigate to time/entities/anchors/etc. that are set in a [`re_uri::Fragment`].
    SetUrlFragment {
        store_id: StoreId,

        /// Uri fragment, see [`re_uri::Fragment`] on how to parse it.
        // Not using `re_uri::Fragment` to avoid further dependency entanglement.
        fragment: String, //re_uri::Fragment,
    },
}
