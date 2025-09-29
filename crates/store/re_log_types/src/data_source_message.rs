// TODO(andreas): Conceptually these should go to `re_data_source`.
// However, `re_data_source` depends on everything that _implements_ a datasource, therefore we would get a circular dependency!

use crate::{AbsoluteTimeRange, LogMsg, StoreId, TimelineName};

/// Message from a data source.
///
/// May contain limited UI commands for instrumenting the state of the receiving end.
#[derive(Clone, Debug)]
pub enum DataSourceMessage {
    /// See [`LogMsg`].
    LogMsg(LogMsg),

    /// A UI command that has to be sorted relative to `LogMsg`s.
    ///
    /// Non-ui receivers can safely ignore these.
    UiCommand(DataSourceUiCommand),
}

impl From<LogMsg> for DataSourceMessage {
    #[inline]
    fn from(msg: LogMsg) -> Self {
        Self::LogMsg(msg)
    }
}

impl From<DataSourceUiCommand> for DataSourceMessage {
    #[inline]
    fn from(cmd: DataSourceUiCommand) -> Self {
        Self::UiCommand(cmd)
    }
}

/// UI commands issued when streaming in datasets.
///
/// If you're not in a ui context you can safely ignore these.
#[derive(Clone, Debug)]
pub enum DataSourceUiCommand {
    AddValidTimeRange {
        store_id: StoreId,

        /// If `None`, signals that all timelines are entirely valid.
        timeline: Option<TimelineName>,
        time_range: AbsoluteTimeRange,
    },

    SetUrlFragment {
        store_id: StoreId,

        /// Uri fragment, see [`re_uri::Fragment`] on how to parse it.
        // Not using `re_uri::Fragment` to avoid further dependency entanglement.
        fragment: String, //re_uri::Fragment,
    },
}
