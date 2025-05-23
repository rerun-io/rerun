use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{ComponentPath, DataPath, EntityPath, EntryId, TableId};

use crate::{ContainerId, Contents, ViewId};

/// One "thing" in the UI.
///
/// This is the granularity of what is selectable and hoverable.
#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Item {
    /// Select a specific application, to see which recordings and blueprints are loaded for it.
    AppId(re_log_types::ApplicationId),

    /// A place where data comes from, e.g. the path to a .rrd or a gRPC URL.
    DataSource(re_smart_channel::SmartChannelSource),

    /// A recording (or blueprint)
    StoreId(re_log_types::StoreId),

    /// A table (i.e. a dataframe)
    TableId(TableId),

    /// An entity or instance from the chunk store.
    InstancePath(InstancePath),

    /// A component of an entity from the chunk store.
    ComponentPath(ComponentPath),

    /// A viewport container.
    Container(ContainerId),

    /// A viewport view.
    View(ViewId),

    /// An entity or instance in the context of a view's data results.
    DataResult(ViewId, InstancePath),

    /// A dataset or table.
    RedapEntry(EntryId),

    /// A Redap server.
    RedapServer(re_uri::Origin),
}

impl Item {
    pub fn entity_path(&self) -> Option<&EntityPath> {
        match self {
            Self::AppId(_)
            | Self::TableId(_)
            | Self::DataSource(_)
            | Self::View(_)
            | Self::Container(_)
            | Self::StoreId(_)
            | Self::RedapServer(_)
            | Self::RedapEntry(_) => None,

            Self::ComponentPath(component_path) => Some(&component_path.entity_path),

            Self::InstancePath(instance_path) | Self::DataResult(_, instance_path) => {
                Some(&instance_path.entity_path)
            }
        }
    }
}

impl From<ViewId> for Item {
    #[inline]
    fn from(view_id: ViewId) -> Self {
        Self::View(view_id)
    }
}

impl From<ComponentPath> for Item {
    #[inline]
    fn from(component_path: ComponentPath) -> Self {
        Self::ComponentPath(component_path)
    }
}

impl From<EntityPath> for Item {
    #[inline]
    fn from(entity_path: EntityPath) -> Self {
        Self::InstancePath(InstancePath::from(entity_path))
    }
}

impl From<InstancePath> for Item {
    #[inline]
    fn from(instance_path: InstancePath) -> Self {
        Self::InstancePath(instance_path)
    }
}

impl From<Contents> for Item {
    #[inline]
    fn from(contents: Contents) -> Self {
        match contents {
            Contents::Container(container_id) => Self::Container(container_id),
            Contents::View(view_id) => Self::View(view_id),
        }
    }
}

impl std::str::FromStr for Item {
    type Err = re_log_types::PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DataPath {
            entity_path,
            instance,
            component_descriptor,
        } = DataPath::from_str(s)?;

        match (instance, component_descriptor) {
            (Some(instance), Some(_component_descriptor)) => {
                // TODO(emilk): support selecting a specific component of a specific instance.
                Err(re_log_types::PathParseError::UnexpectedInstance(instance))
            }
            (Some(instance), None) => Ok(Self::InstancePath(InstancePath::instance(
                entity_path,
                instance,
            ))),
            (None, Some(component_descriptor)) => Ok(Self::ComponentPath(ComponentPath {
                entity_path,
                component_descriptor,
            })),
            (None, None) => Ok(Self::InstancePath(InstancePath::entity_all(entity_path))),
        }
    }
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppId(app_id) => app_id.fmt(f),
            Self::TableId(table_id) => table_id.fmt(f),
            Self::DataSource(data_source) => data_source.fmt(f),
            Self::StoreId(store_id) => store_id.fmt(f),
            Self::ComponentPath(s) => s.fmt(f),
            Self::View(s) => write!(f, "{s:?}"),
            Self::InstancePath(path) => write!(f, "{path}"),
            Self::DataResult(view_id, instance_path) => {
                write!(f, "({view_id:?}, {instance_path}")
            }
            Self::Container(tile_id) => write!(f, "(tile: {tile_id:?})"),
            Self::RedapEntry(entry_id) => write!(f, "{entry_id}"),
            Self::RedapServer(server) => write!(f, "{server}"),
        }
    }
}

impl Item {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AppId(_) => "Application",
            Self::TableId(_) => "Table",
            Self::DataSource(_) => "Data source",
            Self::StoreId(store_id) => match store_id.kind {
                re_log_types::StoreKind::Recording => "Recording ID",
                re_log_types::StoreKind::Blueprint => "Blueprint ID",
            },
            Self::InstancePath(instance_path) => instance_path.kind(),
            Self::ComponentPath(_) => "Entity component",
            Self::View(_) => "View",
            Self::Container(_) => "Container",
            Self::DataResult(_, instance_path) => {
                if instance_path.instance.is_specific() {
                    "Data result instance"
                } else {
                    "Data result entity"
                }
            }
            Self::RedapEntry(_) => "Redap entry",
            Self::RedapServer(_) => "Redap server",
        }
    }
}

/// If the given item refers to the first element of an instance with a single element, resolve to a unindexed entity path.
pub fn resolve_mono_instance_path_item(
    entity_db: &EntityDb,
    query: &re_chunk_store::LatestAtQuery,
    item: &Item,
) -> Item {
    // Resolve to entity path if there's only a single instance.
    match item {
        Item::InstancePath(instance_path) => {
            Item::InstancePath(resolve_mono_instance_path(entity_db, query, instance_path))
        }
        Item::DataResult(view_id, instance_path) => Item::DataResult(
            *view_id,
            resolve_mono_instance_path(entity_db, query, instance_path),
        ),
        Item::AppId(_)
        | Item::TableId(_)
        | Item::DataSource(_)
        | Item::StoreId(_)
        | Item::ComponentPath(_)
        | Item::View(_)
        | Item::Container(_)
        | Item::RedapEntry(_)
        | Item::RedapServer(_) => item.clone(),
    }
}

/// If the given path refers to the first element of an instance with a single element, resolve to a unindexed entity path.
pub fn resolve_mono_instance_path(
    entity_db: &EntityDb,
    query: &re_chunk_store::LatestAtQuery,
    instance: &re_entity_db::InstancePath,
) -> re_entity_db::InstancePath {
    re_tracing::profile_function!();

    if instance.instance.get() == 0 {
        let engine = entity_db.storage_engine();

        // NOTE: While we normally frown upon direct queries to the datastore, `all_components` is fine.
        let Some(component_descrs) = engine
            .store()
            .all_components_on_timeline(&query.timeline(), &instance.entity_path)
        else {
            // No components at all, return unindexed entity.
            return re_entity_db::InstancePath::entity_all(instance.entity_path.clone());
        };

        for component_descr in &component_descrs {
            if let Some(array) = engine
                .cache()
                .latest_at(query, &instance.entity_path, [component_descr])
                .component_batch_raw(component_descr)
            {
                if array.len() > 1 {
                    return instance.clone();
                }
            }
        }

        // All instances had only a single element or less, resolve to unindexed entity.
        return re_entity_db::InstancePath::entity_all(instance.entity_path.clone());
    }

    instance.clone()
}
