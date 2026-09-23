// TODO(andreas): Conceptually these should go to `re_data_source`.
// However, `re_data_source` depends on everything that _implements_ a datasource, therefore we would get a circular dependency!

use std::sync::Arc;

use re_chunk_index::RrdManifest;
use re_log_msg::{LogMsg, TableMsg};
use re_log_types::{ApplicationId, StoreId, impl_into_enum};

use crate::ViewerControlCommand;

/// Message from a data source.
///
/// May contain limited viewer-control commands for instrumenting the state of the receiving end.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub enum DataSourceMessage {
    /// A piece of the index of all the chunks in a recording.
    ///
    /// Some sources may send this, others may not.
    /// There may be one or more of these, followed by [`Self::RrdManifestComplete`].
    RrdManifest(StoreId, Arc<RrdManifest>),

    /// All parts of the RRD manifest have been sent.
    RrdManifestComplete(StoreId),

    /// See [`LogMsg`].
    LogMsg(LogMsg),

    /// Associate a fully received blueprint with one of its consumers.
    // TODO(andreas): Whenever we make the request for a blueprint we should just keep the information
    // about why we pulled it in the first place and therefore should know upon arrival what to do with it?
    // TODO(andreas): If needed, make this more flexible than just making a thing the default.
    DefaultBlueprintRegistration(DefaultBlueprintRegistration),

    /// See [`TableMsg`].
    TableMsg(TableMsg),

    /// A viewer-control command that has to be ordered relative to [`LogMsg`]s.
    ///
    /// Non-ui receivers can safely ignore these.
    // TODO(RR-5073): Remove viewer-control commands from DataSourceMessage
    ViewerControl(ViewerControlCommand),
}

impl_into_enum!(LogMsg, DataSourceMessage, LogMsg);
impl_into_enum!(
    DefaultBlueprintRegistration,
    DataSourceMessage,
    DefaultBlueprintRegistration
);
impl_into_enum!(TableMsg, DataSourceMessage, TableMsg);
impl_into_enum!(ViewerControlCommand, DataSourceMessage, ViewerControl);

impl DataSourceMessage {
    /// The name of the variant, useful for error message etc
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::RrdManifest(..) => "RrdManifest",
            Self::RrdManifestComplete(_) => "RrdManifestComplete",
            Self::LogMsg(_) => "LogMsg",
            Self::DefaultBlueprintRegistration(_) => "BlueprintRegistration",
            Self::TableMsg(_) => "TableMsg",
            Self::ViewerControl(_) => "ViewerControl",
        }
    }

    /// Records the current time as the moment the carried data passed `location`.
    ///
    /// Only messages that carry Arrow data are stamped.
    pub fn track_latency(&mut self, location: re_sorbet::TimestampLocation) {
        match self {
            Self::LogMsg(log_msg) => log_msg.track_latency(location),
            Self::TableMsg(table_msg) => table_msg.track_latency(location),
            Self::RrdManifest(..)
            | Self::RrdManifestComplete(_)
            | Self::DefaultBlueprintRegistration(_)
            | Self::ViewerControl(_) => {}
        }
    }
}

/// An ordered association command sent after all data for a blueprint store.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct DefaultBlueprintRegistration {
    pub blueprint_id: StoreId,
    pub target: BlueprintTarget,
}

/// The consumer of a blueprint.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub enum BlueprintTarget {
    Application(ApplicationId),
    Table(re_uri::TableReference),
}
