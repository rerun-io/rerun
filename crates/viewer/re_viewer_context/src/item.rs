use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{ComponentPath, DataPath, EntityPath, TableId};
use re_sdk_types::blueprint::components::VisualizerInstructionId;

use crate::{BlueprintId, ContainerId, Contents, ViewId};
use crate::{blueprint_id::ViewIdRegistry, open_url::EXAMPLES_ORIGIN};

/// `Item` state for a dataresult interaction, i.e. when hovering or selecting an item in a view's data results.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DataResultInteractionAddress {
    /// The view in which the interaction happened.
    pub view_id: ViewId,

    /// The instance path of the entity or instance that is being interacted with.
    ///
    /// Note that this may be an individual instance or the entire entity if the instance index is [`re_log_types::Instance::ALL`].
    pub instance_path: InstancePath,

    /// Optional visualizer instruction id through which we're interacting with this data-result.
    ///
    /// This can be used for more fine grained highlights.
    /// If not present, we generally assume we're interacting with the dataresult as a whole.
    pub visualizer: Option<VisualizerInstructionId>,
}

impl DataResultInteractionAddress {
    /// Creates a new address for an entity path (all instances, no visualizer).
    pub fn from_entity_path(view_id: ViewId, entity_path: EntityPath) -> Self {
        Self {
            view_id,
            instance_path: InstancePath::entity_all(entity_path),
            visualizer: None,
        }
    }

    /// Returns a new address that refers to the entire entity (all instances),
    /// preserving the view and visualizer.
    pub fn as_entity_all(&self) -> Self {
        Self {
            view_id: self.view_id,
            instance_path: InstancePath::entity_all(self.instance_path.entity_path.clone()),
            visualizer: self.visualizer,
        }
    }
}

/// One "thing" in the UI.
///
/// This is the granularity of what is selectable and hoverable.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Item {
    /// Select a specific application, to see which recordings and blueprints are loaded for it.
    AppId(re_log_types::ApplicationId),

    /// A place where data comes from, e.g. the path to a .rrd or a gRPC URL.
    DataSource(re_log_channel::LogSource),

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
    DataResult(DataResultInteractionAddress),

    /// A table or dataset entry stored in a Redap server.
    // TODO(ab): this should probably be split into separate variant, and made more consistent with
    // `AppId` and `TableId`.
    RedapEntry(re_uri::EntryUri),

    /// A Redap server.
    RedapServer(re_uri::Origin),
}

impl Item {
    /// The example page / welcome screen
    pub fn welcome_page() -> Self {
        Self::RedapServer(EXAMPLES_ORIGIN.clone())
    }

    pub fn view_id(&self) -> Option<BlueprintId<ViewIdRegistry>> {
        match self {
            Self::AppId(_)
            | Self::DataSource(_)
            | Self::StoreId(_)
            | Self::TableId(_)
            | Self::InstancePath(_)
            | Self::ComponentPath(_)
            | Self::Container(_)
            | Self::RedapEntry(_)
            | Self::RedapServer(_) => None,
            Self::View(view_id) => Some(*view_id),
            Self::DataResult(data_result) => Some(data_result.view_id),
        }
    }

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

            Self::InstancePath(instance_path) => Some(&instance_path.entity_path),
            Self::DataResult(data_result) => Some(&data_result.instance_path.entity_path),
        }
    }

    /// Converts this item to a data path if possible.
    pub fn to_data_path(&self) -> Option<DataPath> {
        match self {
            Self::AppId(_)
            | Self::TableId(_)
            | Self::DataSource(_)
            | Self::View(_)
            | Self::Container(_)
            | Self::StoreId(_)
            | Self::RedapServer(_)
            | Self::RedapEntry(_) => None,

            Self::ComponentPath(component_path) => Some(DataPath {
                entity_path: component_path.entity_path.clone(),
                instance: None,
                component: Some(component_path.component),
            }),

            Self::InstancePath(instance_path) => Some(DataPath {
                entity_path: instance_path.entity_path.clone(),
                instance: Some(instance_path.instance),
                component: None,
            }),
            Self::DataResult(data_result) => Some(DataPath {
                entity_path: data_result.instance_path.entity_path.clone(),
                instance: Some(data_result.instance_path.instance),
                component: None,
            }),
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
            component,
        } = DataPath::from_str(s)?;

        match (instance, component) {
            (Some(instance), Some(_component_descriptor)) => {
                // TODO(emilk): support selecting a specific component of a specific instance.
                Err(re_log_types::PathParseError::UnexpectedInstance(instance))
            }
            (Some(instance), None) => Ok(Self::InstancePath(InstancePath::instance(
                entity_path,
                instance,
            ))),
            (None, Some(component)) => Ok(Self::ComponentPath(ComponentPath {
                entity_path,
                component,
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
            Self::DataResult(data_result) => {
                write!(
                    f,
                    "({:?}, {})",
                    data_result.view_id, data_result.instance_path
                )
            }
            Self::Container(tile_id) => write!(f, "(tile: {tile_id:?})"),
            Self::RedapEntry(entry) => {
                write!(f, "{entry}")
            }
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
            Self::StoreId(store_id) => match store_id.kind() {
                re_log_types::StoreKind::Recording => "Recording ID",
                re_log_types::StoreKind::Blueprint => "Blueprint ID",
            },
            Self::InstancePath(instance_path) => instance_path.kind(),
            Self::ComponentPath(_) => "Entity component",
            Self::View(_) => "View",
            Self::Container(_) => "Container",
            Self::DataResult(data_result) => {
                if data_result.instance_path.instance.is_specific() {
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
        Item::DataResult(data_result) => Item::DataResult(DataResultInteractionAddress {
            instance_path: resolve_mono_instance_path(entity_db, query, &data_result.instance_path),
            ..data_result.clone()
        }),
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
        let Some(components) = engine
            .store()
            .all_components_on_timeline(&query.timeline(), &instance.entity_path)
        else {
            // No components at all, return unindexed entity.
            return re_entity_db::InstancePath::entity_all(instance.entity_path.clone());
        };

        #[expect(clippy::iter_over_hash_type)]
        for component in components {
            if let Some(array) = engine
                .cache()
                .latest_at(query, &instance.entity_path, [component])
                .component_batch_raw(component)
                && array.len() > 1
            {
                return instance.clone();
            }
        }

        // All instances had only a single element or less, resolve to unindexed entity.
        return re_entity_db::InstancePath::entity_all(instance.entity_path.clone());
    }

    instance.clone()
}
