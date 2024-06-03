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
    let path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());
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

        let blueprint_store_path = entity_path_for_view_property_from_archetype_name(
            space_view_id,
            blueprint_db.tree(),
            archetype_name,
        );

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

// TODO(andreas): Replace all usages with `ViewProperty`.
pub fn entity_path_for_view_property<T: Archetype>(
    space_view_id: SpaceViewId,
    _blueprint_entity_tree: &EntityTree,
) -> EntityPath {
    entity_path_for_view_property_from_archetype_name(
        space_view_id,
        _blueprint_entity_tree,
        T::name(),
    )
}

fn entity_path_for_view_property_from_archetype_name(
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

// TODO(andreas): Replace all usages with `ViewProperty`.
/// Return the archetype value for the given space view, or `None` if it doesn't exist.
pub fn view_property<A: re_types::Archetype>(
    ctx: &re_viewer_context::ViewerContext<'_>,
    space_view_id: re_viewer_context::SpaceViewId,
) -> Option<A>
where
    LatestAtResults: ToArchetype<A>,
{
    let blueprint_db = ctx.blueprint_db();
    let blueprint_query = ctx.blueprint_query;
    let path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());
    blueprint_db
        .latest_at_archetype(&path, blueprint_query)
        .ok()
        .flatten()
        .map(|(_index, value)| value)
}

// TODO(andreas): Replace all usages with `ViewProperty`.
pub fn query_view_property_or_default<A: Archetype + Default>(
    space_view_id: SpaceViewId,
    blueprint_db: &EntityDb,
    query: &LatestAtQuery,
) -> (A, EntityPath)
where
    LatestAtResults: ToArchetype<A>,
{
    let (arch, path) = query_view_property(space_view_id, blueprint_db, query);
    (arch.ok().flatten().unwrap_or_default(), path)
}

// TODO(andreas): Replace all usages with `ViewProperty`.
/// Edit a single component of a blueprint archetype in a space view.
///
/// Set to `None` to reset the value to the value in the default blueprint, if any,
/// else will just store `None` (an empty component list) in the store.
pub fn edit_blueprint_component<A: re_types::Archetype, C: re_types::Component + PartialEq, R>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    edit_component: impl FnOnce(&mut Option<C>) -> R,
) -> R {
    let active_blueprint = ctx.blueprint_db();
    let active_path = entity_path_for_view_property::<A>(space_view_id, active_blueprint.tree());
    let original_value: Option<C> = active_blueprint
        .latest_at_component::<C>(&active_path, ctx.blueprint_query)
        .map(|x| x.value);

    let mut edited_value = original_value.clone();
    let ret = edit_component(&mut edited_value);

    if edited_value != original_value {
        if let Some(edited) = edited_value {
            ctx.save_blueprint_component(&active_path, &edited);
        } else {
            // Reset to the value in the default blueprint, if any.
            ctx.reset_blueprint_component_by_name(&active_path, C::name());
        }
    }

    ret
}
