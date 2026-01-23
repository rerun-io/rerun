// TODO(andreas): Conceptually these should go to `re_data_source`.
// However, `re_data_source` depends on everything that _implements_ a datasource, therefore we would get a circular dependency!

use re_log_encoding::RrdManifest;
use re_log_types::{LogMsg, StoreId, TableMsg, impl_into_enum};

/// Message from a data source.
///
/// May contain limited UI commands for instrumenting the state of the receiving end.
#[derive(Clone, Debug)]
pub enum DataSourceMessage {
    /// The index of all the chunks in a recording.
    ///
    /// Some sources may send this, others may not.
    RrdManifest(StoreId, Box<RrdManifest>),

    /// See [`LogMsg`].
    LogMsg(LogMsg),

    /// See [`TableMsg`].
    TableMsg(TableMsg),

    /// A UI command that has to be ordered relative to [`LogMsg`]s.
    ///
    /// Non-ui receivers can safely ignore these.
    UiCommand(DataSourceUiCommand),
}

impl re_byte_size::SizeBytes for DataSourceMessage {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::RrdManifest(_, manifest) => manifest.heap_size_bytes(),
            Self::LogMsg(log_msg) => log_msg.heap_size_bytes(),
            Self::TableMsg(table_msg) => table_msg.heap_size_bytes(),
            Self::UiCommand(_) => 0,
        }
    }
}

impl_into_enum!(LogMsg, DataSourceMessage, LogMsg);
impl_into_enum!(TableMsg, DataSourceMessage, TableMsg);
impl_into_enum!(DataSourceUiCommand, DataSourceMessage, UiCommand);

impl DataSourceMessage {
    /// The name of the variant, useful for error message etc
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::RrdManifest { .. } => "RrdManifest",
            Self::LogMsg(_) => "LogMsg",
            Self::TableMsg(_) => "TableMsg",
            Self::UiCommand(_) => "UiCommand",
        }
    }

    // We sometimes inject meta-data for latency tracking etc
    pub fn insert_arrow_record_batch_metadata(&mut self, key: String, value: String) {
        match self {
            Self::LogMsg(log_msg) => log_msg.insert_arrow_record_batch_metadata(key, value),
            Self::TableMsg(table_msg) => table_msg.insert_arrow_record_batch_metadata(key, value),
            Self::RrdManifest { .. } | Self::UiCommand(_) => {
                // Not everything needs latency tracking
            }
        }
    }
}

/// UI commands issued when streaming in datasets.
///
/// If you're not in a ui context you can safely ignore these.
#[derive(Clone, Debug)]
pub enum DataSourceUiCommand {
    /// Navigate to time/entities/anchors/etc. that are set in a `re_uri::Fragment`.
    SetUrlFragment {
        store_id: StoreId,

        /// Uri fragment, see `re_uri::Fragment` on how to parse it.
        // Not using `re_uri::Fragment` to avoid further dependency entanglement.
        fragment: String, //re_uri::Fragment,
    },

    /// Save a screenshot to a file.
    SaveScreenshot {
        /// File path to save the screenshot to.
        // TODO(#12482): Returning the screenshot to the caller would be more flexible and useful.
        file_path: camino::Utf8PathBuf,

        /// Optional view id to screenshot a specific view.
        ///
        /// If none is provided, the entire viewer is screenshotted.
        view_id: Option<String>,
    },
}

impl re_byte_size::SizeBytes for DataSourceUiCommand {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::SetUrlFragment { store_id, fragment } => {
                store_id.heap_size_bytes() + fragment.heap_size_bytes()
            }
            Self::SaveScreenshot { file_path, view_id } => {
                file_path.capacity() as u64 + view_id.heap_size_bytes()
            }
        }
    }
}
