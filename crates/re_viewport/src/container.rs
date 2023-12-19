use egui_tiles::TileId;
use re_arrow_store::LatestAtQuery;
use re_data_store::StoreDb;
use re_log_types::{DataRow, EntityPath, RowId, TimePoint, Timeline};
use re_query::query_archetype;
use re_types_core::ArrowBuffer;
use re_viewer_context::{
    ContainerId, SpaceViewId, SystemCommand, SystemCommandSender as _, ViewerContext,
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
    pub tile_id: Option<egui_tiles::TileId>,
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
            tile_id,
        } = query_archetype(blueprint_db.store(), &query, &id.as_entity_path())
            .and_then(|arch| arch.to_archetype())
            .map_err(|err| {
                if cfg!(debug_assertions) {
                    re_log::error!("Failed to load Container blueprint: {err}.");
                } else {
                    re_log::debug!("Failed to load Container blueprint: {err}.");
                }
                err
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

        let tile_id = tile_id.map(|id| id.0);

        Some(Self {
            id,
            container_kind,
            display_name,
            contents,
            primary_weights,
            secondary_weights,
            tile_id,
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

        let mut arch = crate::blueprint::archetypes::ContainerBlueprint::new(self.container_kind)
            .with_display_name(self.display_name.clone())
            .with_contents(&contents)
            .with_primary_weights(primary_weights)
            .with_secondary_weights(secondary_weights);

        if let Some(tile_id) = self.tile_id {
            arch = arch.with_tile_id(tile_id);
        }

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
        tile_id: TileId,
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
                tile_id: Some(tile_id),
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
                    tile_id: Some(tile_id),
                }
            }
            egui_tiles::Container::Grid(grid) => Self {
                id: container_id,
                container_kind: egui_tiles::ContainerKind::Grid,
                display_name: format!("{:?}", egui_tiles::ContainerKind::Grid),
                contents,
                primary_weights: grid.col_shares.clone(),
                secondary_weights: grid.row_shares.clone(),
                tile_id: Some(tile_id),
            },
        }
    }

    pub fn to_empty_tile_container(&self) -> egui_tiles::Container {
        match self.container_kind {
            egui_tiles::ContainerKind::Tabs => egui_tiles::Container::new_tabs(vec![]),
            egui_tiles::ContainerKind::Horizontal => egui_tiles::Container::new_horizontal(vec![]),
            egui_tiles::ContainerKind::Vertical => egui_tiles::Container::new_vertical(vec![]),
            egui_tiles::ContainerKind::Grid => egui_tiles::Container::new_grid(vec![]),
        }
    }
}
