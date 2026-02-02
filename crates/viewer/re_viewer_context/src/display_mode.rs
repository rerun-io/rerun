use re_log_types::{StoreId, TableId};

use crate::Item;

/// Which display mode are we currently in?
#[derive(Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// The settings dialog for application-wide configuration.
    ///
    /// The inner display mode is the one to return to when exiting this mode.
    Settings(Box<Self>),

    // TODO(isse): It would be nice to only switch to newly loaded items if we
    // are on the loading screen for that specific item.
    /// A loading screen to some source.
    Loading(Box<re_log_channel::LogSource>),

    /// Regular view of the local recordings, including the current recording's viewport.
    LocalRecordings(StoreId),

    LocalTable(TableId),

    /// The Redap server/catalog/collection browser.
    RedapEntry(re_uri::EntryUri),
    RedapServer(re_uri::Origin),

    /// The current recording's data store browser.
    ///
    /// The inner display mode is the one to return to when exiting this mode.
    ChunkStoreBrowser(Box<Self>),
}

impl std::fmt::Debug for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings(_) => write!(f, "Settings"),
            Self::Loading(source) => write!(f, "Loading({source})"),
            Self::LocalRecordings(store_id) => write!(f, "LocalRecordings({store_id:?})"),
            Self::LocalTable(table_id) => write!(f, "LocalTable({table_id})"),
            Self::RedapEntry(uri) => write!(f, "RedapEntry({uri})"),
            Self::RedapServer(origin) => write!(f, "RedapServer({origin})"),
            Self::ChunkStoreBrowser(_) => write!(f, "ChunkStoreBrowser"),
        }
    }
}

// TODO(grtlr,ab): This needs to be further cleaned up and split into separately handled
// display modes. See https://www.notion.so/rerunio/Major-refactor-of-re_viewer-1d8b24554b198085a02dfe441db330b4
impl DisplayMode {
    pub fn has_blueprint_panel(&self) -> bool {
        !matches!(self, Self::Settings(_) | Self::ChunkStoreBrowser(_))
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
            Self::ChunkStoreBrowser(mode) => mode.item(),
            Self::Settings(_) | Self::Loading(_) => None,
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

    pub fn redap_origin(&self, store_hub: &crate::StoreHub) -> Option<re_uri::Origin> {
        match self {
            Self::LocalTable(_) => None,

            Self::LocalRecordings(store_id) => {
                let db = store_hub.entity_db(store_id)?;
                let source = db.data_source.as_ref()?;
                let uri = source.redap_uri()?;

                Some(uri.origin().clone())
            }

            Self::Settings(d) | Self::ChunkStoreBrowser(d) => d.redap_origin(store_hub),

            Self::Loading(log_source) => log_source.redap_uri().map(|r| r.origin().clone()),
            Self::RedapEntry(entry) => Some(entry.origin.clone()),
            Self::RedapServer(origin) => Some(origin.clone()),
        }
    }
}
