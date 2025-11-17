use crate::Item;
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

    pub fn item(&self) -> Option<Item> {
        match self {
            Self::LocalRecordings(store_id) => Some(Item::StoreId(store_id.clone())),
            Self::LocalTable(table_id) => Some(Item::TableId(table_id.clone())),
            Self::RedapEntry(entry_uri) => Some(Item::RedapEntry(entry_uri.clone())),
            Self::RedapServer(origin) => Some(Item::RedapServer(origin.clone())),
            Self::Settings | Self::Loading(_) | Self::ChunkStoreBrowser => None,
        }
    }

    pub fn from_item(item: &crate::Item) -> Option<Self> {
        match item {
            Item::StoreId(store_id) => Some(Self::LocalRecordings(store_id.clone())),
            Item::TableId(table_id) => Some(Self::LocalTable(table_id.clone())),
            Item::RedapEntry(entry_uri) => Some(Self::RedapEntry(entry_uri.clone())),
            Item::RedapServer(origin) => Some(Self::RedapServer(origin.clone())),

            Item::AppId(_)
            | Item::DataSource(_)
            | Item::InstancePath(_)
            | Item::ComponentPath(_)
            | Item::Container(_)
            | Item::View(_)
            | Item::DataResult(_, _) => None,
        }
    }
}
