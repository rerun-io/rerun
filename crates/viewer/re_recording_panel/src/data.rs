//! Data structures describing the contents of the recording panel.

use std::collections::BTreeMap;
use std::iter;
use std::sync::Arc;
use std::task::Poll;

use ahash::HashMap;
use itertools::{Either, Itertools as _};
use re_entity_db::EntityDb;
use re_entity_db::entity_db::EntityDbClass;
use re_log_channel::LogSource;
use re_log_types::{ApplicationId, EntryId, TableId, natural_ordering};
use re_redap_browser::{Entries, EntryInner, RedapServers};
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::{Name, Timestamp};
use re_viewer_context::{DisplayMode, Item, ViewerContext};

/// Short-lived structure containing all the data that will be displayed in the recording panel.
#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct RecordingPanelData<'a> {
    /// All the configured servers.
    pub servers: Vec<ServerData<'a>>,

    /// All the locally loaded application IDs and the corresponding recordings.
    pub local_apps: Vec<AppIdData<'a>>,

    /// All the locally loaded tables.
    pub local_tables: Vec<TableId>,

    /// All the loaded examples
    pub example_apps: Vec<AppIdData<'a>>,

    /// Should the example section be displayed at all?
    pub show_example_section: bool,

    /// Recordings that are currently being loaded that we cannot attribute yet to a specific
    /// section.
    pub loading_receivers: Vec<Arc<LogSource>>,
}

impl<'a> RecordingPanelData<'a> {
    pub fn new(ctx: &'a ViewerContext<'a>, servers: &'a RedapServers, hide_examples: bool) -> Self {
        re_tracing::profile_function!();

        //
        // Find relevant loading sources
        //

        let mut loading_receivers = vec![];
        let mut loading_segments: HashMap<re_uri::Origin, HashMap<EntryId, Vec<Arc<LogSource>>>> =
            HashMap::default();

        let sources_with_stores: ahash::HashSet<LogSource> = ctx
            .storage_context
            .bundle
            .recordings()
            .filter_map(|store| store.data_source.clone())
            .collect();

        for source in ctx.connected_receivers.sources() {
            if sources_with_stores.contains(&source) {
                continue;
            }

            match source.as_ref() {
                LogSource::File(_) | LogSource::HttpStream { .. } => {
                    loading_receivers.push(source);
                }

                LogSource::RedapGrpcStream { uri, .. } => {
                    loading_segments
                        .entry(uri.origin.clone())
                        .or_default()
                        .entry(EntryId::from(uri.dataset_id))
                        .or_default()
                        .push(source);
                }

                // We only show things we know are very-soon-to-be recordings, which these are not.
                LogSource::RrdWebEvent
                | LogSource::JsChannel { .. }
                | LogSource::Sdk
                | LogSource::Stdin
                | LogSource::MessageProxy(_) => {}
            }
        }

        //
        // Find everything else
        //

        let servers = servers
            .iter_servers()
            .map(|server| ServerData::new(ctx, server, loading_segments.get(server.origin())))
            .collect();

        let mut local_apps: BTreeMap<ApplicationId, Vec<&EntityDb>> = Default::default();
        let mut examples_apps: BTreeMap<ApplicationId, Vec<&EntityDb>> = Default::default();

        for entity_db in ctx.storage_context.bundle.entity_dbs() {
            let app_id = entity_db.application_id();
            match entity_db.store_class() {
                EntityDbClass::LocalRecording => local_apps
                    .entry(app_id.clone())
                    .or_default()
                    .push(entity_db),

                EntityDbClass::ExampleRecording => examples_apps
                    .entry(app_id.clone())
                    .or_default()
                    .push(entity_db),

                // these are either handled elsewhere or ignored
                EntityDbClass::DatasetSegment(_) | EntityDbClass::Blueprint => {}
            }
        }

        let local_apps = local_apps
            .into_iter()
            .map(|(app_id_or_examples, entity_dbs)| {
                AppIdData::new(ctx, app_id_or_examples, entity_dbs)
            })
            .collect();

        let example_apps: Vec<_> = examples_apps
            .into_iter()
            .map(|(app_id_or_examples, entity_dbs)| {
                AppIdData::new(ctx, app_id_or_examples, entity_dbs)
            })
            .collect();

        let show_example_section = ctx
            .app_options()
            .include_rerun_examples_button_in_recordings_panel
            && !hide_examples
            || !example_apps.is_empty();

        let local_tables = ctx
            .storage_context
            .tables
            .keys()
            .sorted()
            .cloned()
            .collect();

        Self {
            servers,
            local_apps,
            local_tables,
            example_apps,
            show_example_section,
            loading_receivers,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.local_apps.is_empty()
            && self.local_tables.is_empty()
            && self.example_apps.is_empty()
            && self.servers.is_empty()
    }

    /// Search for the relevant store id and, if found, return its sibling entity dbs and its index
    /// within them.
    pub fn collection_from_recording(
        &'a self,
        store_id: &re_log_types::StoreId,
    ) -> Option<(usize, Vec<&'a EntityDb>)> {
        for server in &self.servers {
            for dataset in server.entries_data.iter_datasets() {
                let store_iter = dataset.iter_loaded_stores();

                if let Some(pos) = store_iter.clone().position(|db| db.store_id() == store_id) {
                    return Some((pos, store_iter.collect()));
                }
            }
        }

        for local_app in self.local_apps.iter().chain(self.example_apps.iter()) {
            let store_iter = local_app.iter_loaded_stores();

            if let Some(pos) = store_iter.clone().position(|db| db.store_id() == store_id) {
                return Some((pos, store_iter.collect()));
            }
        }

        None
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct AppIdData<'a> {
    pub app_id: ApplicationId,
    pub is_active: bool,
    pub is_selected: bool,

    pub loaded_recordings: Vec<RecordingData<'a>>,
}

impl<'a> AppIdData<'a> {
    fn new(
        ctx: &'a ViewerContext<'a>,
        app_id: ApplicationId,
        mut entity_dbs: Vec<&'a EntityDb>,
    ) -> Self {
        entity_dbs.sort_by_cached_key(|entity_db| {
            (
                entity_db
                    .recording_info_property::<Name>(RecordingInfo::descriptor_name().component)
                    .map(|s| natural_ordering::OrderedString(s.to_string())),
                entity_db.recording_info_property::<Timestamp>(
                    RecordingInfo::descriptor_start_time().component,
                ),
            )
        });

        let is_active = false;
        let is_selected = ctx.is_selected_or_loading(&Item::AppId(app_id.clone()));

        let loaded_recordings = entity_dbs
            .into_iter()
            .map(|entity_db| RecordingData { entity_db })
            .collect();

        Self {
            app_id,
            is_active,
            is_selected,
            loaded_recordings,
        }
    }

    pub fn id(&self) -> egui::Id {
        egui::Id::new(&self.app_id)
    }

    pub fn name(&self) -> &str {
        self.app_id.as_str()
    }

    pub fn item(&self) -> Item {
        Item::AppId(self.app_id.clone())
    }

    pub fn iter_loaded_stores(&'a self) -> impl Iterator<Item = &'a EntityDb> + Clone {
        self.loaded_recordings.iter().map(|rec| rec.entity_db)
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct RecordingData<'a> {
    #[cfg_attr(feature = "testing", serde(serialize_with = "serialize_entity_db"))]
    pub entity_db: &'a EntityDb,
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct ServerData<'a> {
    pub origin: re_uri::Origin,
    pub is_active: bool,
    pub is_selected: bool,

    pub entries_data: ServerEntriesData<'a>,
}

impl<'a> ServerData<'a> {
    fn new(
        ctx: &'a ViewerContext<'_>,
        server: &re_redap_browser::Server,
        loading_segments: Option<&HashMap<EntryId, Vec<Arc<LogSource>>>>,
    ) -> Self {
        let origin = server.origin();
        let item = Item::RedapServer(origin.clone());

        let is_selected = ctx.is_selected_or_loading(&item);
        let is_active = matches!(
            ctx.display_mode(),
            DisplayMode::RedapServer(current_origin)
            if current_origin == origin
        );

        let entries_data = ServerEntriesData::new(ctx, server.entries(), origin, loading_segments);

        Self {
            origin: origin.clone(),
            is_active,
            is_selected,
            entries_data,
        }
    }

    pub fn item(&self) -> Item {
        Item::RedapServer(self.origin.clone())
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub enum ServerEntriesData<'a> {
    Loading,

    Error {
        message: String,
        is_auth_error: bool,
    },

    Loaded {
        dataset_entries: Vec<DatasetData<'a>>,
        table_entries: Vec<RemoteTableData>,
        failed_entries: Vec<FailedEntryData>,
    },
}

impl<'a> ServerEntriesData<'a> {
    fn new(
        ctx: &'a ViewerContext<'a>,
        entries: &Entries,
        origin: &re_uri::Origin,
        loading_segments: Option<&HashMap<EntryId, Vec<Arc<LogSource>>>>,
    ) -> Self {
        match entries.state() {
            Poll::Ready(Ok(entries)) => {
                let mut dataset_entries = vec![];
                let mut table_entries = vec![];
                let mut failed_entries = vec![];

                for entry in entries.values().sorted_by_key(|entry| entry.name()) {
                    let entry_data = EntryData {
                        origin: origin.clone(),
                        entry_id: entry.id(),
                        name: entry.name().to_owned(),
                        icon: entry.icon(),
                        is_selected: ctx.is_selected_or_loading(&Item::RedapEntry(
                            re_uri::EntryUri {
                                origin: origin.clone(),
                                entry_id: entry.id(),
                            },
                        )),
                        is_active: ctx.active_redap_entry() == Some(entry.id()),
                    };

                    match entry.inner() {
                        Ok(EntryInner::Dataset(_dataset)) => {
                            let mut displayed_segments: Vec<SegmentData<'_>> = ctx
                                .storage_context
                                .bundle
                                .entity_dbs()
                                .filter_map(|entity_db| {
                                    if let EntityDbClass::DatasetSegment(uri) =
                                        entity_db.store_class()
                                    {
                                        if &uri.origin == origin
                                            && EntryId::from(uri.dataset_id) == entry.id()
                                        {
                                            Some(SegmentData::Loaded { entity_db })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            if let Some(loading_segments) = loading_segments
                                && let Some(smart_channels) = loading_segments.get(&entry.id())
                            {
                                displayed_segments.extend(smart_channels.iter().map(|source| {
                                    SegmentData::Loading {
                                        receiver: source.clone(),
                                    }
                                }));
                            }

                            displayed_segments.sort_by_key(|segment| match segment {
                                SegmentData::Loading { receiver } => {
                                    ctx.storage_context.hub.data_source_order(receiver)
                                }
                                SegmentData::Loaded { entity_db } => {
                                    if let Some(data_source) = &entity_db.data_source {
                                        ctx.storage_context.hub.data_source_order(data_source)
                                    } else {
                                        u64::MAX
                                    }
                                }
                            });

                            dataset_entries.push(DatasetData {
                                entry_data,
                                displayed_segments,
                            });
                        }

                        Ok(EntryInner::Table(_table)) => {
                            table_entries.push(RemoteTableData { entry_data });
                        }

                        Err(err) => failed_entries.push(FailedEntryData {
                            entry_data,
                            error: err.to_string(),
                        }),
                    }
                }

                Self::Loaded {
                    dataset_entries,
                    table_entries,
                    failed_entries,
                }
            }

            Poll::Ready(Err(err)) => Self::Error {
                message: err.to_string(),
                is_auth_error: err.is_client_credentials_error(),
            },

            Poll::Pending => Self::Loading,
        }
    }

    pub fn iter_datasets(&'a self) -> impl Iterator<Item = &'a DatasetData<'a>> {
        match self {
            Self::Loaded {
                dataset_entries, ..
            } => Either::Left(dataset_entries.iter()),

            Self::Error { .. } | Self::Loading => Either::Right(iter::empty()),
        }
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct DatasetData<'a> {
    pub entry_data: EntryData,
    pub displayed_segments: Vec<SegmentData<'a>>,
}

impl<'a> DatasetData<'a> {
    pub fn iter_loaded_stores(&'a self) -> impl Iterator<Item = &'a EntityDb> + Clone {
        self.displayed_segments
            .iter()
            .filter_map(|segment| match segment {
                SegmentData::Loaded { entity_db } => Some(*entity_db),
                SegmentData::Loading { .. } => None,
            })
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct RemoteTableData {
    pub entry_data: EntryData,
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct FailedEntryData {
    pub entry_data: EntryData,
    pub error: String,
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub struct EntryData {
    pub origin: re_uri::Origin,
    pub entry_id: re_log_types::EntryId,

    pub name: String,

    #[cfg_attr(feature = "testing", serde(serialize_with = "serialize_icon"))]
    pub icon: re_ui::icons::Icon,

    pub is_selected: bool,
    pub is_active: bool,
}

impl EntryData {
    pub fn item(&self) -> Item {
        Item::RedapEntry(self.entry_uri())
    }

    pub fn id(&self) -> egui::Id {
        egui::Id::new(&self.origin)
            .with(self.entry_id)
            .with(&self.name)
    }

    pub fn entry_uri(&self) -> re_uri::EntryUri {
        re_uri::EntryUri {
            origin: self.origin.clone(),
            entry_id: self.entry_id,
        }
    }
}

// ---

#[derive(Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize))]
pub enum SegmentData<'a> {
    Loading {
        receiver: Arc<LogSource>,
    },
    Loaded {
        #[cfg_attr(feature = "testing", serde(serialize_with = "serialize_entity_db"))]
        entity_db: &'a EntityDb,
    },
}

impl SegmentData<'_> {
    pub fn entity_db(&self) -> Option<&EntityDb> {
        match self {
            SegmentData::Loaded { entity_db, .. } => Some(entity_db),
            SegmentData::Loading { .. } => None,
        }
    }
}

// ---

#[cfg(feature = "testing")]
fn serialize_entity_db<S>(value: &EntityDb, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize as _;
    value.store_id().serialize(serializer)
}

#[cfg(feature = "testing")]
fn serialize_icon<S>(value: &re_ui::Icon, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize as _;
    value.uri().serialize(serializer)
}
