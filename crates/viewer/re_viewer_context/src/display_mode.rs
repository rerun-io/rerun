use re_log_types::{ApplicationId, StoreId, TableId};

use crate::{Item, open_url::EXAMPLES_ORIGIN};

/// What are we currently showing in the viewer?
//
// TODO(RR-3033): Rename to `Route`
#[derive(Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// The settings dialog for application-wide configuration.
    Settings {
        /// What to return to when exiting this mode.
        previous: Box<Self>,
    },

    // TODO(isse): It would be nice to only switch to newly loaded items if we
    // are on the loading screen for that specific item.
    /// A loading screen to some source.
    Loading(Box<re_log_channel::LogSource>),

    /// Regular view of the local recordings, including the current recording's viewport.
    LocalRecording {
        recording_id: StoreId,
        // TODO(RR-3033): add blueprint id
    },

    LocalTable(TableId),

    /// The Redap server/catalog/collection browser.
    RedapEntry(re_uri::EntryUri),

    /// The top-level view of a Redap Server
    ///
    /// Also used for the example/welcome page, using [`EXAMPLES_ORIGIN`].
    RedapServer(re_uri::Origin),

    /// A debug-view into the raw chunks of a recording.
    ChunkStoreBrowser {
        recording_id: StoreId,

        /// What to return to when exiting this mode.
        previous: Box<Self>,
    },
}

impl std::fmt::Debug for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings { .. } => write!(f, "Settings"),
            Self::Loading(source) => write!(f, "Loading({source})"),
            Self::LocalRecording { recording_id } => write!(f, "LocalRecording({recording_id:?})"),
            Self::LocalTable(table_id) => write!(f, "LocalTable({table_id})"),
            Self::RedapEntry(uri) => write!(f, "RedapEntry({uri})"),
            Self::RedapServer(origin) => write!(f, "RedapServer({origin})"),
            Self::ChunkStoreBrowser { recording_id, .. } => {
                write!(f, "ChunkStoreBrowser({recording_id:?})")
            }
        }
    }
}

// TODO(grtlr,ab): This needs to be further cleaned up and split into separately handled
// display modes. See https://www.notion.so/rerunio/Major-refactor-of-re_viewer-1d8b24554b198085a02dfe441db330b4
impl DisplayMode {
    /// The example page / welcome screen
    pub fn welcome_page() -> Self {
        Self::RedapServer(EXAMPLES_ORIGIN.clone())
    }

    /// The active recording [`StoreId`], if any.
    pub fn recording_id(&self) -> Option<&StoreId> {
        match self {
            Self::LocalRecording { recording_id }
            | Self::ChunkStoreBrowser { recording_id, .. } => Some(recording_id),
            Self::Settings { .. }
            | Self::Loading { .. }
            | Self::LocalTable { .. }
            | Self::RedapEntry { .. }
            | Self::RedapServer { .. } => None,
        }
    }

    // TODO(RR-3033): remove this app-id centric world
    pub fn app_id(&self) -> Option<&ApplicationId> {
        match self {
            Self::LocalRecording { recording_id }
            | Self::ChunkStoreBrowser { recording_id, .. } => Some(recording_id.application_id()),
            Self::Settings { previous } => previous.app_id(),
            Self::RedapServer(origin) => {
                if origin == &*EXAMPLES_ORIGIN {
                    Some(crate::StoreHub::welcome_screen_app_id())
                } else {
                    None
                }
            }
            Self::Loading { .. } | Self::LocalTable { .. } | Self::RedapEntry { .. } => None,
        }
    }

    pub fn has_blueprint_panel(&self) -> bool {
        !matches!(self, Self::Settings { .. } | Self::ChunkStoreBrowser { .. })
    }

    pub fn has_selection_panel(&self) -> bool {
        matches!(self, Self::LocalRecording { .. })
    }

    pub fn has_time_panel(&self) -> bool {
        matches!(self, Self::LocalRecording { .. })
    }

    pub fn item(&self) -> Option<Item> {
        match self {
            Self::LocalRecording { recording_id }
            | Self::ChunkStoreBrowser { recording_id, .. } => {
                Some(Item::StoreId(recording_id.clone()))
            }
            Self::LocalTable(table_id) => Some(Item::TableId(table_id.clone())),
            Self::RedapEntry(entry_uri) => Some(Item::RedapEntry(entry_uri.clone())),
            Self::RedapServer(origin) => Some(Item::RedapServer(origin.clone())),
            Self::Settings { .. } | Self::Loading { .. } => None,
        }
    }

    pub fn from_item(item: &crate::Item) -> Option<Self> {
        match item {
            Item::StoreId(store_id) => Some(Self::LocalRecording {
                recording_id: store_id.clone(),
            }),
            Item::TableId(table_id) => Some(Self::LocalTable(table_id.clone())),
            Item::RedapEntry(entry_uri) => Some(Self::RedapEntry(entry_uri.clone())),
            Item::RedapServer(origin) => Some(Self::RedapServer(origin.clone())),

            Item::AppId { .. }
            | Item::DataSource { .. }
            | Item::InstancePath { .. }
            | Item::ComponentPath { .. }
            | Item::Container { .. }
            | Item::View { .. }
            | Item::DataResult { .. } => None,
        }
    }

    /// Returns the redap origin for the current display mode, if any.
    ///
    /// Proxy origins are excluded because they are local and don't represent
    /// a remote server connection.
    pub fn redap_origin(&self, store_hub: &crate::StoreHub) -> Option<re_uri::Origin> {
        match self {
            Self::LocalTable { .. } => None,

            Self::LocalRecording { recording_id }
            | Self::ChunkStoreBrowser { recording_id, .. } => {
                let db = store_hub.entity_db(recording_id)?;
                let source = db.data_source.as_ref()?;
                let uri = source.redap_uri()?;

                // Don't return proxy origins — they are local.
                if matches!(uri, re_uri::RedapUri::Proxy(_)) {
                    return None;
                }

                Some(uri.origin().clone())
            }

            Self::Settings { previous } => previous.redap_origin(store_hub),

            Self::Loading(log_source) => {
                let uri = log_source.redap_uri()?;

                // Don't return proxy origins — they are local.
                if matches!(uri, re_uri::RedapUri::Proxy(_)) {
                    return None;
                }

                Some(uri.origin().clone())
            }
            Self::RedapEntry(entry) => Some(entry.origin.clone()),
            Self::RedapServer(origin) => Some(origin.clone()),
        }
    }
}
