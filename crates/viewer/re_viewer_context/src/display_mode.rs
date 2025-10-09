use re_log_types::{StoreId, TableId};

/// Which display mode are we currently in?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// The settings dialog for application-wide configuration.
    Settings,

    // TODO(isse): It would be nice to only switch to newly loaded items if we
    // are on the loading screen for that specific item.
    /// A loading screen to some source.
    Loading(Box<re_smart_channel::SmartChannelSource>),

    /// Regular view of the local recordings, including the current recording's viewport.
    LocalRecordings(StoreId),

    LocalTable(TableId),

    /// The Redap server/catalog/collection browser.
    RedapEntry(re_uri::EntryUri),
    RedapServer(re_uri::Origin),

    /// The current recording's data store browser.
    ChunkStoreBrowser,
}
