use re_chunk::ChunkId;
use re_log_types::{ApplicationId, EntryId, StoreId, TableId};

use crate::{Item, RedapEntryKind, TableReference, open_url::EXAMPLES_ORIGIN};

/// What a redap entry is, and for a dataset which of its resources we show.
///
/// The server is the authority on this, see `re_protos`' `EntryKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryKind {
    Table,
    Dataset(re_uri::DatasetResource),
}

/// What are we currently showing in the viewer?
#[derive(Clone, PartialEq, Eq)]
pub enum Route {
    /// The settings dialog for application-wide configuration.
    Settings {
        /// What to return to when exiting this mode.
        return_route: Box<Self>,
    },

    // TODO(isse): It would be nice to only switch to newly loaded items if we
    // are on the loading screen for that specific item.
    /// A loading screen to some source.
    Loading(Box<re_log_channel::LogSource>),

    /// Regular view of the local recordings, including the current recording's viewport.
    ///
    /// This includes recordings we're streaming from a Redap server.
    LocalRecording {
        recording_id: StoreId,
    },

    LocalTable(TableId),

    /// A dataset or table entry on a Redap server.
    RedapEntry {
        origin: re_uri::Origin,
        entry_id: EntryId,

        /// What the entry is, or `None` while we don't know.
        ///
        /// A route built from an entry url or an [`Item`] has nothing to go by.
        kind: Option<EntryKind>,
    },

    /// A folder in a Redap server's dataset hierarchy, named by a dotted path prefix.
    RedapFolder {
        origin: re_uri::Origin,
        path: String,
    },

    /// The top-level view of a Redap Server.
    ///
    /// Also used for the example/welcome page, using [`EXAMPLES_ORIGIN`].
    RedapServer(re_uri::Origin),

    /// A debug-view into the raw chunks of a store (recording or blueprint).
    ChunkStoreBrowser {
        /// The store to browse. `None` when opened from a route that has
        /// no active store — the chunk browser's internal picker will
        /// let the user choose one.
        store_id: Option<StoreId>,
        selected_chunk: Option<ChunkId>,

        /// What to return to when exiting this mode.
        return_route: Box<Self>,
    },
}

impl std::fmt::Debug for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings { .. } => write!(f, "Settings"),
            Self::Loading(source) => write!(f, "Loading({source})"),
            Self::LocalRecording { recording_id } => write!(f, "LocalRecording({recording_id:?})"),
            Self::LocalTable(table_id) => write!(f, "LocalTable({table_id})"),
            Self::RedapEntry {
                origin,
                entry_id,
                kind,
            } => write!(f, "RedapEntry({origin}, {entry_id}, {kind:?})"),
            Self::RedapFolder { origin, path } => write!(f, "RedapFolder({origin}, {path})"),
            Self::RedapServer(server) => write!(f, "RedapServer({server})"),
            Self::ChunkStoreBrowser {
                store_id,
                selected_chunk,
                ..
            } => {
                write!(f, "ChunkStoreBrowser({store_id:?}, {selected_chunk:?})")
            }
        }
    }
}

impl Route {
    /// The example page / welcome screen
    pub fn welcome_page() -> Self {
        Self::RedapServer(EXAMPLES_ORIGIN.clone())
    }

    /// The active recording [`StoreId`], if any.
    pub fn recording_id(&self) -> Option<&StoreId> {
        match self {
            Self::LocalRecording { recording_id } => Some(recording_id),

            Self::ChunkStoreBrowser { .. } // `store_id` of the chunk store browser is just what it shows and may be a blueprint!
            | Self::Settings { .. }
            | Self::Loading { .. }
            | Self::LocalTable { .. }
            | Self::RedapEntry { .. }
            | Self::RedapFolder { .. }
            | Self::RedapServer { .. } => None,
        }
    }

    // TODO(andreas): remove this app-id centric world.
    // We use this mostly for blueprint association which is very brittle and not very well defined right now. See also RR-3033.
    pub fn app_id(&self) -> Option<&ApplicationId> {
        match self {
            Self::LocalRecording { recording_id } => Some(recording_id.application_id()),
            Self::ChunkStoreBrowser { store_id, .. } => {
                store_id.as_ref().map(StoreId::application_id)
            }
            Self::Settings { return_route } => return_route.app_id(),
            Self::RedapServer(server) => {
                if server == &*EXAMPLES_ORIGIN {
                    Some(crate::StoreHub::welcome_screen_app_id())
                } else {
                    None
                }
            }
            Self::Loading { .. }
            | Self::LocalTable { .. }
            | Self::RedapEntry { .. }
            | Self::RedapFolder { .. } => None,
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

    /// The entry this route shows, if any.
    pub fn entry_id(&self) -> Option<EntryId> {
        match self {
            Self::RedapEntry { entry_id, .. } => Some(*entry_id),

            Self::Settings { .. }
            | Self::Loading { .. }
            | Self::LocalRecording { .. }
            | Self::LocalTable { .. }
            | Self::RedapFolder { .. }
            | Self::RedapServer { .. }
            | Self::ChunkStoreBrowser { .. } => None,
        }
    }

    pub fn item(&self) -> Option<Item> {
        match self {
            Self::LocalRecording { recording_id } => Some(Item::StoreId(recording_id.clone())),
            Self::ChunkStoreBrowser { store_id, .. } => {
                store_id.as_ref().map(|id| Item::StoreId(id.clone()))
            }
            Self::LocalTable(table_id) => Some(Item::TableId(table_id.clone())),
            Self::RedapEntry {
                origin,
                entry_id,
                kind: _,
            } => Some(Item::RedapEntry {
                origin: origin.clone(),
                kind: RedapEntryKind::Entry(*entry_id),
            }),
            Self::RedapFolder { origin, path } => Some(Item::RedapEntry {
                origin: origin.clone(),
                kind: RedapEntryKind::Folder(path.clone()),
            }),
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
            Item::RedapEntry { origin, kind } => match kind {
                RedapEntryKind::Entry(entry_id) => Some(Self::RedapEntry {
                    origin: origin.clone(),
                    entry_id: *entry_id,
                    kind: None,
                }),
                RedapEntryKind::Folder(path) => Some(Self::RedapFolder {
                    origin: origin.clone(),
                    path: path.clone(),
                }),
            },
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
}

impl From<re_uri::EntryUri> for Route {
    fn from(uri: re_uri::EntryUri) -> Self {
        Self::RedapEntry {
            origin: uri.origin,
            entry_id: uri.entry_id,
            kind: None,
        }
    }
}

impl Route {
    /// Returns the referenced table, if any.
    pub fn table_reference(&self) -> Option<TableReference> {
        match self {
            Self::LocalTable(table_id) => Some(table_id.clone().into()),

            // The same table backs every resource of a dataset.
            Self::RedapEntry {
                origin, entry_id, ..
            } => Some(TableReference::RedapEntry {
                origin: origin.clone(),
                entry_id: *entry_id,
            }),

            Self::RedapServer(origin) => Some(TableReference::RedapServerEntries {
                origin: origin.clone(),
            }),

            Self::Settings { .. }
            | Self::Loading { .. }
            | Self::LocalRecording { .. }
            | Self::RedapFolder { .. }
            | Self::ChunkStoreBrowser { .. } => None,
        }
    }

    /// Returns the redap origin for the current route, if any.
    ///
    /// Proxy origins are excluded because they are local and do not represent a remote server connection.
    pub fn redap_origin(&self, store_hub: &crate::StoreHub) -> Option<re_uri::Origin> {
        match self {
            Self::LocalRecording { recording_id }
            | Self::ChunkStoreBrowser {
                store_id: Some(recording_id),
                ..
            } => {
                let db = store_hub.entity_db(recording_id)?;
                let source = db.data_source.as_ref()?;
                let uri = source.redap_uri()?;

                // Don't return proxy origins — they are local.
                if matches!(uri, re_uri::RedapUri::Proxy(_)) {
                    return None;
                }

                Some(uri.origin().clone())
            }

            Self::Settings { return_route }
            | Self::ChunkStoreBrowser {
                store_id: None,
                return_route,
                ..
            } => return_route.redap_origin(store_hub),

            Self::Loading(log_source) => {
                let uri = log_source.redap_uri()?;

                // Don't return proxy origins — they are local.
                if matches!(uri, re_uri::RedapUri::Proxy(_)) {
                    return None;
                }

                Some(uri.origin().clone())
            }
            Self::RedapEntry { origin, .. } | Self::RedapFolder { origin, .. } => {
                Some(origin.clone())
            }
            Self::RedapServer(server) => Some(server.clone()),

            Self::LocalTable { .. } => None,
        }
    }
}
