use re_data_store::LatestAtQuery;
use re_entity_db::{
    external::re_query::{LatestAtResults, PromiseResult, ToArchetype},
    EntityDb,
};
use re_log_types::EntityPath;
use re_types::{external::arrow2, Archetype, ArchetypeName, ComponentName};
use re_viewer_context::{
    external::re_entity_db::EntityTree, ComponentFallbackError, ComponentFallbackProvider,
    QueryContext, SpaceViewId, SpaceViewSystemExecutionError, ViewerContext,
};

// TODO(andreas): Replace all usages with `ViewProperty`.
/// Returns `Ok(None)` if any of the required components are missing.
pub fn query_view_property<A: Archetype>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (PromiseResult<Option<A>>, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let path = entity_path_for_view_property(space_view_id, blueprint_db.tree(), A::name());
    (
        blueprint_db
            .latest_at_archetype(&path, query)
            .map(|res| res.map(|(_, arch)| arch)),
        path,
    )
}

#[derive(thiserror::Error, Debug)]
pub enum ViewPropertyQueryError {
    #[error(transparent)]
    SerializationError(#[from] re_types::DeserializationError),

    #[error(transparent)]
    ComponentFallbackError(#[from] ComponentFallbackError),
}

impl From<ViewPropertyQueryError> for SpaceViewSystemExecutionError {
    fn from(val: ViewPropertyQueryError) -> Self {
        match val {
            ViewPropertyQueryError::SerializationError(err) => err.into(),
            ViewPropertyQueryError::ComponentFallbackError(err) => err.into(),
        }
    }
}

/// Utility for querying view properties.
pub struct ViewProperty<'a> {
    blueprint_store_path: EntityPath,
    archetype_name: ArchetypeName,
    query_results: LatestAtResults,
    viewer_ctx: &'a ViewerContext<'a>,
}

impl<'a> ViewProperty<'a> {
    /// Query a specific view property for a given view.
    pub fn from_archetype<A: Archetype>(
        viewer_ctx: &'a ViewerContext<'a>,
        view_id: SpaceViewId,
    ) -> Self {
        Self::from_archetype_impl(viewer_ctx, view_id, A::name(), A::all_components().as_ref())
    }

    fn from_archetype_impl(
        viewer_ctx: &'a ViewerContext<'a>,
        space_view_id: SpaceViewId,
        archetype_name: ArchetypeName,
        component_names: &[ComponentName],
    ) -> Self {
        let blueprint_db = viewer_ctx.blueprint_db();

        let blueprint_store_path =
            entity_path_for_view_property(space_view_id, blueprint_db.tree(), archetype_name);

        let query_results = blueprint_db.latest_at(
            viewer_ctx.blueprint_query,
            &blueprint_store_path,
            component_names.iter().copied(),
        );

        ViewProperty {
            blueprint_store_path,
            archetype_name,
            query_results,

            viewer_ctx,
        }
    }

    /// Get the value of a specific component or its fallback if the component is not present.
    // TODO(andreas): Unfortunately we can't use TypedComponentFallbackProvider here because it may not be implemented for all components of interest.
    // This sadly means that there's a bit of unnecessary back and forth between arrow array and untyped that could be avoided otherwise.
    pub fn component_or_fallback<C: re_types::Component + Default>(
        &self,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &'a dyn re_viewer_context::SpaceViewState,
    ) -> Result<C, ViewPropertyQueryError> {
        self.component_array_or_fallback::<C>(fallback_provider, view_state)?
            .into_iter()
            .next()
            .ok_or(ComponentFallbackError::UnexpectedEmptyFallback.into())
    }

    /// Get the value of a specific component or its fallback if the component is not present.
    pub fn component_array_or_fallback<C: re_types::Component + Default>(
        &self,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &'a dyn re_viewer_context::SpaceViewState,
    ) -> Result<Vec<C>, ViewPropertyQueryError> {
        let component_name = C::name();
        Ok(C::from_arrow(
            self.component_or_fallback_raw(component_name, fallback_provider, view_state)?
                .as_ref(),
        )?)
    }

    fn component_raw(
        &self,
        component_name: ComponentName,
    ) -> Option<Box<dyn arrow2::array::Array>> {
        self.query_results.get(component_name).and_then(|result| {
            result.raw(self.viewer_ctx.blueprint_db().resolver(), component_name)
        })
    }

    fn component_or_fallback_raw(
        &self,
        component_name: ComponentName,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &'a dyn re_viewer_context::SpaceViewState,
    ) -> Result<Box<dyn arrow2::array::Array>, ComponentFallbackError> {
        if let Some(value) = self.component_raw(component_name) {
            if value.len() > 0 {
                return Ok(value);
            }
        }
        fallback_provider.fallback_for(&self.query_context(view_state), component_name)
    }

    /// Save change to a blueprint component.
    pub fn save_blueprint_component<C: re_types::Component>(&self, component: &C) {
        self.viewer_ctx
            .save_blueprint_component(&self.blueprint_store_path, component);
    }

    /// Resets a blueprint component to the value it had in the default blueprint.
    pub fn reset_blueprint_component<C: re_types::Component>(&self) {
        self.viewer_ctx
            .reset_blueprint_component_by_name(&self.blueprint_store_path, C::name());
    }

    fn query_context(
        &self,
        view_state: &'a dyn re_viewer_context::SpaceViewState,
    ) -> QueryContext<'_> {
        QueryContext {
            viewer_ctx: self.viewer_ctx,
            target_entity_path: &self.blueprint_store_path,
            archetype_name: Some(self.archetype_name),
            query: self.viewer_ctx.blueprint_query,
            view_state,
        }
    }
}

pub fn entity_path_for_view_property(
    space_view_id: SpaceViewId,
    _blueprint_entity_tree: &EntityTree,
    archetype_name: ArchetypeName,
) -> EntityPath {
    // TODO(andreas,jleibs):
    // We want to search the subtree for occurrences of the property archetype here.
    // Only if none is found we make up a new (standardized) path.
    // There's some nuances to figure out what happens when we find the archetype several times.
    // Also, we need to specify what it means to "find" the archetype (likely just matching the indicator?).
    let space_view_blueprint_path = space_view_id.as_entity_path();

    // Use short_name instead of full_name since full_name has dots and looks too much like an indicator component.
    space_view_blueprint_path.join(&EntityPath::from_single_string(archetype_name.short_name()))
}

