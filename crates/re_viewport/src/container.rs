use ahash::HashMap;
use egui_tiles::TileId;
use re_data_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log::ResultExt;
use re_log_types::{DataRow, EntityPath, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_types_core::{archetypes::Clear, ArrowBuffer};
use re_viewer_context::{
    BlueprintId, BlueprintIdRegistry, ContainerId, SpaceViewId, SystemCommand,
    SystemCommandSender as _, ViewerContext,
};

#[derive(Clone, Debug)]
pub enum Contents {
    Container(ContainerId),
    SpaceView(SpaceViewId),
}

impl Contents {
    fn try_from(path: &EntityPath) -> Option<Self> {
        if path.starts_with(SpaceViewId::registry()) {
            Some(Self::SpaceView(SpaceViewId::from_entity_path(path)))
        } else if path.starts_with(ContainerId::registry()) {
            Some(Self::Container(ContainerId::from_entity_path(path)))
        } else {
            None
        }
    }

    #[inline]
    fn to_entity_path(&self) -> EntityPath {
        match self {
            Self::Container(id) => id.as_entity_path(),
            Self::SpaceView(id) => id.as_entity_path(),
        }
    }

    #[inline]
    fn to_tile_id(&self) -> TileId {
        match self {
            Self::Container(id) => blueprint_id_to_tile_id(id),
            Self::SpaceView(id) => blueprint_id_to_tile_id(id),
        }
    }

    #[inline]
    pub fn as_container_id(&self) -> Option<ContainerId> {
        match self {
            Self::Container(id) => Some(*id),
            Self::SpaceView(_) => None,
        }
    }

    #[inline]
    pub fn as_space_view_id(&self) -> Option<SpaceViewId> {
        match self {
            Self::SpaceView(id) => Some(*id),
            Self::Container(_) => None,
        }
    }
}

#[inline]
pub fn blueprint_id_to_tile_id<T: BlueprintIdRegistry>(id: &BlueprintId<T>) -> TileId {
    TileId::from_u64(id.hash())
}

impl From<SpaceViewId> for Contents {
    #[inline]
    fn from(id: SpaceViewId) -> Self {
        Self::SpaceView(id)
    }
}

impl From<ContainerId> for Contents {
    #[inline]
    fn from(id: ContainerId) -> Self {
        Self::Container(id)
    }
}

/// The native version of a [`crate::blueprint::archetypes::ContainerBlueprint`].
///
/// This represents a single container in the blueprint. On each frame, it is
/// used to populate an [`egui_tiles::Container`]. Each child in `contents` can
/// be either a [`SpaceViewId`] or another [`ContainerId`].
///
/// The main reason this exists is to handle type conversions that aren't yet
/// well handled by the code-generated archetypes.
#[derive(Debug)]
pub struct ContainerBlueprint {
    pub id: ContainerId,
    pub container_kind: egui_tiles::ContainerKind,
    pub display_name: String,
    pub contents: Vec<Contents>,
    pub primary_weights: Vec<f32>,
    pub secondary_weights: Vec<f32>,
    pub active_tab: Option<Contents>,
}

impl ContainerBlueprint {
    /// Attempt to load a [`ContainerBlueprint`] from the blueprint store.
    pub fn try_from_db(blueprint_db: &EntityDb, id: ContainerId) -> Option<Self> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());

        let crate::blueprint::archetypes::ContainerBlueprint {
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
            active_tab,
        } = query_archetype(blueprint_db.store(), &query, &id.as_entity_path())
            .and_then(|arch| arch.to_archetype())
            .map_err(|err| {
                if !matches!(err, re_query::QueryError::PrimaryNotFound(_)) {
                    if cfg!(debug_assertions) {
                        re_log::error!("Failed to load Container blueprint: {err}.");
                    } else {
                        re_log::debug!("Failed to load Container blueprint: {err}.");
                    }
                }
            })
            .ok()?;

        let container_kind = container_kind.into();

        // TODO(jleibs): Don't use debug print for this
        let display_name =
            display_name.map_or_else(|| format!("{container_kind:?}"), |v| v.0.to_string());

        let contents = contents
            .unwrap_or_default()
            .0
            .into_iter()
            .filter_map(|id| Contents::try_from(&id.into()))
            .collect();

        let primary_weights = primary_weights
            .unwrap_or_default()
            .0
            .iter()
            .cloned()
            .collect();

        let secondary_weights = secondary_weights
            .unwrap_or_default()
            .0
            .iter()
            .cloned()
            .collect();

        let active_tab = active_tab.and_then(|id| Contents::try_from(&id.0.into()));

        Some(Self {
            id,
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
            active_tab,
        })
    }

    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    /// Persist the entire [`ContainerBlueprint`] to the blueprint store.
    ///
    /// This only needs to be called if the [`ContainerBlueprint`] was created with [`Self::from_egui_tiles_container`].
    ///
    /// Otherwise, incremental calls to `set_` functions will write just the necessary component
    /// update directly to the store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let timepoint = TimePoint::timeless();

        let contents: Vec<_> = self
            .contents
            .iter()
            .map(|item| item.to_entity_path())
            .collect();

        let primary_weights: ArrowBuffer<_> = self.primary_weights.clone().into();
        let secondary_weights: ArrowBuffer<_> = self.secondary_weights.clone().into();

        let mut arch = crate::blueprint::archetypes::ContainerBlueprint::new(self.container_kind)
            .with_display_name(self.display_name.clone())
            .with_contents(&contents)
            .with_primary_weights(primary_weights)
            .with_secondary_weights(secondary_weights);

        // TODO(jleibs): The need for this pattern is annoying. Should codegen
        // a version of this that can take an Option.
        if let Some(active_tab) = &self.active_tab {
            arch = arch.with_active_tab(&active_tab.to_entity_path());
        }

        let mut deltas = vec![];

        if let Some(row) =
            DataRow::from_archetype(RowId::new(), timepoint.clone(), self.entity_path(), &arch)
                .warn_on_err_once("Failed to create Container blueprint.")
        {
            deltas.push(row);

            ctx.command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    ctx.store_context.blueprint.store_id().clone(),
                    deltas,
                ));
        }
    }

    /// Creates a new [`ContainerBlueprint`] from the given [`egui_tiles::Container`].
    ///
    /// This [`ContainerBlueprint`] is ephemeral. If you want to make it permanent you
    /// must call [`Self::save_to_blueprint_store`].
    pub fn from_egui_tiles_container(
        container_id: ContainerId,
        container: &egui_tiles::Container,
        tile_to_contents: &HashMap<TileId, Contents>,
    ) -> Self {
        let contents = container
            .children()
            .filter_map(|child_id| {
                tile_to_contents.get(child_id).cloned().or_else(|| {
                    re_log::warn_once!("Missing child when building container.");
                    None
                })
            })
            .collect();

        match container {
            egui_tiles::Container::Tabs(tab) => {
                let active_tab = tab.active.and_then(|id| tile_to_contents.get(&id).cloned());

                Self {
                    id: container_id,
                    container_kind: egui_tiles::ContainerKind::Tabs,
                    display_name: format!("{:?}", egui_tiles::ContainerKind::Tabs),
                    contents,
                    primary_weights: vec![],
                    secondary_weights: vec![],
                    active_tab,
                }
            }
            egui_tiles::Container::Linear(linear) => {
                // TODO(jleibs): This should be part of egui_tiles.
                let kind = match linear.dir {
                    egui_tiles::LinearDir::Horizontal => egui_tiles::ContainerKind::Horizontal,
                    egui_tiles::LinearDir::Vertical => egui_tiles::ContainerKind::Vertical,
                };
                Self {
                    id: container_id,
                    container_kind: kind,
                    display_name: format!("{kind:?}"),
                    contents,
                    primary_weights: linear
                        .children
                        .iter()
                        .map(|child| linear.shares[*child])
                        .collect(),
                    secondary_weights: vec![],
                    active_tab: None,
                }
            }
            egui_tiles::Container::Grid(grid) => Self {
                id: container_id,
                container_kind: egui_tiles::ContainerKind::Grid,
                display_name: format!("{:?}", egui_tiles::ContainerKind::Grid),
                contents,
                primary_weights: grid.col_shares.clone(),
                secondary_weights: grid.row_shares.clone(),
                active_tab: None,
            },
        }
    }

    /// Clears the blueprint component for this container.
    // TODO(jleibs): Should this be a recursive clear?
    pub fn clear(&self, ctx: &ViewerContext<'_>) {
        let clear = Clear::recursive();
        ctx.save_blueprint_component(&self.entity_path(), clear.is_recursive);
    }

    pub fn to_tile(&self) -> egui_tiles::Tile<SpaceViewId> {
        let children = self
            .contents
            .iter()
            .map(|item| item.to_tile_id())
            .collect::<Vec<_>>();

        let container = match self.container_kind {
            egui_tiles::ContainerKind::Tabs => {
                let mut tabs = egui_tiles::Tabs::new(children);
                tabs.active = self
                    .active_tab
                    .as_ref()
                    .map(|id| id.to_tile_id())
                    .or_else(|| tabs.children.first().copied());
                egui_tiles::Container::Tabs(tabs)
            }
            egui_tiles::ContainerKind::Horizontal | egui_tiles::ContainerKind::Vertical => {
                let linear_dir = match self.container_kind {
                    egui_tiles::ContainerKind::Horizontal => egui_tiles::LinearDir::Horizontal,
                    egui_tiles::ContainerKind::Vertical => egui_tiles::LinearDir::Vertical,
                    _ => unreachable!(),
                };
                let mut linear = egui_tiles::Linear::new(linear_dir, children.clone());

                for (share, id) in self.primary_weights.iter().zip(children.iter()) {
                    linear.shares[*id] = *share;
                }

                egui_tiles::Container::Linear(linear)
            }
            egui_tiles::ContainerKind::Grid => {
                let mut grid = egui_tiles::Grid::new(children);

                grid.col_shares = self.primary_weights.clone();
                grid.row_shares = self.secondary_weights.clone();

                egui_tiles::Container::Grid(grid)
            }
        };

        egui_tiles::Tile::Container(container)
    }
}
