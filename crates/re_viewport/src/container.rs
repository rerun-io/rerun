use egui_tiles::ContainerKind;
use re_arrow_store::LatestAtQuery;
use re_data_store::StoreDb;
use re_log_types::{EntityPath, Timeline};
use re_query::query_archetype;
use re_viewer_context::{ContainerId, SpaceViewId};
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
}
