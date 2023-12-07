use re_arrow_store::{LatestAtQuery, VersionedComponent};
use re_data_store::EntityPath;
use re_log_types::RowId;
use re_types::{
    archetypes::Tensor, components::TensorData, tensor_data::DecodedTensor, Archetype,
    ComponentNameSet,
};
use re_viewer_context::{
    default_heuristic_filter, HeuristicFilterContext, IdentifiedViewSystem,
    SpaceViewSystemExecutionError, TensorDecodeCache, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
};

#[derive(Default)]
pub struct TensorSystem {
    pub tensors: std::collections::BTreeMap<EntityPath, (RowId, DecodedTensor)>,
}

impl IdentifiedViewSystem for TensorSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Tensor".into()
    }
}

impl ViewPartSystem for TensorSystem {
    fn required_components(&self) -> ComponentNameSet {
        Tensor::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Tensor::indicator().name()).collect()
    }

    fn heuristic_filter(
        &self,
        store: &re_arrow_store::DataStore,
        ent_path: &EntityPath,
        _ctx: HeuristicFilterContext,
        query: &LatestAtQuery,
        entity_components: &ComponentNameSet,
    ) -> bool {
        if !default_heuristic_filter(entity_components, &self.indicator_components()) {
            return false;
        }

        // The tensor view can't display anything with less than two dimensions.
        if let Some(tensor) =
            store.query_latest_component::<re_types::components::TensorData>(ent_path, query)
        {
            !tensor.is_vector()
        } else {
            false
        }
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let store = ctx.store_db.store();
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            if let Some(tensor) = store
                .query_latest_component::<TensorData>(&data_result.entity_path, &timeline_query)
            {
                self.load_tensor_entity(ctx, &data_result.entity_path, tensor);
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TensorSystem {
    fn load_tensor_entity(
        &mut self,
        ctx: &ViewerContext<'_>,
        ent_path: &EntityPath,
        tensor: VersionedComponent<TensorData>,
    ) {
        match ctx
            .cache
            .entry(|c: &mut TensorDecodeCache| c.entry(tensor.row_id, tensor.value.0))
        {
            Ok(decoded_tensor) => {
                self.tensors
                    .insert(ent_path.clone(), (tensor.row_id, decoded_tensor));
            }
            Err(err) => {
                re_log::warn_once!("Failed to decode decoding tensor at path {ent_path}: {err}");
            }
        }
    }
}
