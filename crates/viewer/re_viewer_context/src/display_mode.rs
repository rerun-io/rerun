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

// TODO(grtlr,ab): This needs to be further cleaned up and split into separately handled
// display modes. See https://www.notion.so/rerunio/Major-refactor-of-re_viewer-1d8b24554b198085a02dfe441db330b4
impl DisplayMode {
    pub fn has_blueprint_panel(&self) -> bool {
        !matches!(self, Self::Settings | Self::ChunkStoreBrowser)
    }

    pub fn has_selection_panel(&self) -> bool {
        matches!(self, Self::LocalRecordings(_))
    }

    pub fn has_time_panel(&self) -> bool {
        matches!(self, Self::LocalRecordings(_))
    }
}
