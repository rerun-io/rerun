use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_components::Tensor;
use re_data_store::EntityPath;
use re_log_types::Component as _;
use re_viewer_context::{
    ArchetypeDefinition, ScenePart, ScenePartCollection, SceneQuery, SpaceViewHighlights,
    ViewerContext,
};

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneBarChart {
    pub charts: BTreeMap<EntityPath, Tensor>,
}

impl ScenePartCollection for SceneBarChart {
    type Context = ();
    type ScenePartData = ();

    fn vec_mut(&mut self) -> Vec<&mut dyn ScenePart<Self>> {
        vec![self]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ScenePart<SceneBarChart> for SceneBarChart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Tensor::name()]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _scene_context: &(),
        _highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor = store.query_latest_component::<Tensor>(ent_path, &query);

            if let Some(tensor) = tensor {
                if tensor.is_vector() {
                    self.charts.insert(ent_path.clone(), tensor.clone()); // shallow clones
                }
            }
        }

        Vec::new()
    }
}
