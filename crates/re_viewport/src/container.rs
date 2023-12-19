use egui_tiles::TileId;
use re_arrow_store::LatestAtQuery;
use re_data_store::StoreDb;
use re_log_types::{DataRow, EntityPath, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_types_core::{archetypes::Clear, ArrowBuffer};
use re_viewer_context::{
    BlueprintId, BlueprintIdRegistry, ContainerId, SpaceViewId, SystemCommand,
    SystemCommandSender as _, ViewerContext,
};

pub enum ContainerOrSpaceView {
    Container(ContainerId),
    SpaceView(SpaceViewId),
}

impl ContainerOrSpaceView {
    fn try_from(path: &EntityPath) -> Option<Self> {
        path.parent().and_then(|parent| {
            if &parent == SpaceViewId::registry() {
                Some(Self::SpaceView(SpaceViewId::from_entity_path(path)))
            } else if &parent == ContainerId::registry() {
                Some(Self::Container(ContainerId::from_entity_path(path)))
            } else {
                None
            }
        })
    }

    fn to_entity_path(&self) -> EntityPath {
        match self {
            Self::Container(id) => id.as_entity_path(),
            Self::SpaceView(id) => id.as_entity_path(),
        }
    }

    fn to_tile_id(&self) -> TileId {
        match self {
            Self::Container(id) => blueprint_id_to_tile_id(id),
            Self::SpaceView(id) => blueprint_id_to_tile_id(id),
        }
    }
}

pub fn blueprint_id_to_tile_id<T: BlueprintIdRegistry>(id: &BlueprintId<T>) -> TileId {
    // TOOD(jleibs): This conversion to entity path is more expensive than it should be
    let path = id.as_entity_path();

    TileId::from_u64(path.hash64())
}

impl From<SpaceViewId> for ContainerOrSpaceView {
    fn from(id: SpaceViewId) -> Self {
        Self::SpaceView(id)
    }
}

impl From<ContainerId> for ContainerOrSpaceView {
    fn from(id: ContainerId) -> Self {
        Self::Container(id)
    }
}

pub struct ContainerBlueprint {
    pub id: ContainerId,
    pub container_kind: egui_tiles::ContainerKind,
    pub display_name: String,
    pub contents: Vec<ContainerOrSpaceView>,
    pub primary_weights: Vec<f32>,
    pub secondary_weights: Vec<f32>,
}

impl ContainerBlueprint {
    pub fn try_from_db(id: ContainerId, blueprint_db: &StoreDb) -> Option<Self> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());

        let crate::blueprint::archetypes::ContainerBlueprint {
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
        } = query_archetype(blueprint_db.store(), &query, &id.as_entity_path())
            .and_then(|arch| arch.to_archetype())
            // TODO(jleibs): When we clear containers from the store this starts
            // failing to a missing required component -- query_archetype sohuld
            // be able to handle this case gracefully.
            /*
            .map_err(|err| {
                if cfg!(debug_assertions) {
                    re_log::error!("Failed to load Container blueprint: {err}.");
                } else {
                    re_log::debug!("Failed to load Container blueprint: {err}.");
                }
                err
            })
            */
            .ok()?;

        let container_kind = container_kind.into();

        // TODO(jleibs): Don't use debug print for this
        let display_name =
            display_name.map_or_else(|| format!("{container_kind:?}"), |v| v.0.to_string());

        let contents = contents
            .unwrap_or_default()
            .0
            .into_iter()
            .filter_map(|id| ContainerOrSpaceView::try_from(&id.into()))
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

        Some(Self {
            id,
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
        })
    }

    pub fn entity_path(&self) -> EntityPath {
        self.id.as_entity_path()
    }

    /// Persist the entire [`ContainerBlueprint`] to the blueprint store.
    pub fn save_to_blueprint_store(&self, ctx: &ViewerContext<'_>) {
        let timepoint = TimePoint::timeless();

        let contents: Vec<_> = self
            .contents
            .iter()
            .map(|item| item.to_entity_path())
            .collect();

        let primary_weights: ArrowBuffer<_> = self.primary_weights.clone().into();
        let secondary_weights: ArrowBuffer<_> = self.secondary_weights.clone().into();

        let arch = crate::blueprint::archetypes::ContainerBlueprint::new(self.container_kind)
            .with_display_name(self.display_name.clone())
            .with_contents(&contents)
            .with_primary_weights(primary_weights)
            .with_secondary_weights(secondary_weights);

        let mut deltas = vec![];

        if let Ok(row) =
            DataRow::from_archetype(RowId::new(), timepoint.clone(), self.entity_path(), &arch)
        {
            deltas.push(row);
        }

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                deltas,
            ));
    }

    pub fn new(
        container_id: ContainerId,
        contents: Vec<ContainerOrSpaceView>,
        container: &egui_tiles::Container,
    ) -> Self {
        match container {
            egui_tiles::Container::Tabs(_) => Self {
                id: container_id,
                container_kind: egui_tiles::ContainerKind::Tabs,
                display_name: format!("{:?}", egui_tiles::ContainerKind::Tabs),
                contents,
                primary_weights: vec![],
                secondary_weights: vec![],
            },
            egui_tiles::Container::Linear(linear) => {
                // TODO(abey79): This should be part of egui_tiles
                let kind = match linear.dir {
                    egui_tiles::LinearDir::Horizontal => egui_tiles::ContainerKind::Horizontal,
                    egui_tiles::LinearDir::Vertical => egui_tiles::ContainerKind::Vertical,
                };
                Self {
                    id: container_id,
                    container_kind: kind,
                    display_name: format!("{kind:?}"),
                    contents,
                    primary_weights: linear.shares.into_iter().map(|(_, share)| *share).collect(),
                    secondary_weights: vec![],
                }
            }
            egui_tiles::Container::Grid(grid) => Self {
                id: container_id,
                container_kind: egui_tiles::ContainerKind::Grid,
                display_name: format!("{:?}", egui_tiles::ContainerKind::Grid),
                contents,
                primary_weights: grid.col_shares.clone(),
                secondary_weights: grid.row_shares.clone(),
            },
        }
    }

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
                // TODO(abey79): Need to add active tab to the blueprint spec
                tabs.active = tabs.children.first().copied();
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
