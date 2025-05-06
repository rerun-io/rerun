use re_chunk_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtResults, EntityDb};
use re_log_types::EntityPath;
use re_types::{
    Archetype, ArchetypeName, ComponentBatch, ComponentDescriptor, ComponentName,
    DeserializationError,
};
use re_viewer_context::{
    external::re_entity_db::EntityTree, ComponentFallbackError, ComponentFallbackProvider,
    QueryContext, ViewId, ViewSystemExecutionError, ViewerContext,
};

#[derive(thiserror::Error, Debug)]
pub enum ViewPropertyQueryError {
    #[error(transparent)]
    SerializationError(#[from] re_types::DeserializationError),

    #[error(transparent)]
    ComponentFallbackError(#[from] ComponentFallbackError),
}

impl From<ViewPropertyQueryError> for ViewSystemExecutionError {
    fn from(val: ViewPropertyQueryError) -> Self {
        match val {
            ViewPropertyQueryError::SerializationError(err) => err.into(),
            ViewPropertyQueryError::ComponentFallbackError(err) => err.into(),
        }
    }
}

/// Utility for querying view properties.
#[derive(Debug)]
pub struct ViewProperty {
    /// Entity path in the blueprint store where all components of this view property archetype are
    /// stored.
    pub blueprint_store_path: EntityPath,

    /// Name of the property archetype.
    pub archetype_name: ArchetypeName,

    /// List of all components in this property.
    pub component_descrs: Vec<ComponentDescriptor>,

    /// Query results for all queries of this property.
    pub query_results: LatestAtResults,

    /// Blueprint query used for querying.
    pub blueprint_query: LatestAtQuery,
}

impl ViewProperty {
    /// Query a specific view property for a given view.
    pub fn from_archetype<A: Archetype>(
        blueprint_db: &EntityDb,
        blueprint_query: &LatestAtQuery,
        view_id: ViewId,
    ) -> Self {
        Self::from_archetype_impl(
            blueprint_db,
            blueprint_query.clone(),
            view_id,
            A::name(),
            A::all_components().iter().cloned().collect(),
        )
    }

    fn from_archetype_impl(
        blueprint_db: &EntityDb,
        blueprint_query: LatestAtQuery,
        view_id: ViewId,
        archetype_name: ArchetypeName,
        component_descrs: Vec<ComponentDescriptor>,
    ) -> Self {
        let blueprint_store_path =
            entity_path_for_view_property(view_id, blueprint_db.tree(), archetype_name);

        let query_results = blueprint_db.latest_at(
            &blueprint_query,
            &blueprint_store_path,
            component_descrs.iter().map(|descr| descr.component_name),
        );

        Self {
            blueprint_store_path,
            archetype_name,
            query_results,
            component_descrs,
            blueprint_query,
        }
    }

    /// Get the value of a specific component or its fallback if the component is not present.
    // TODO(andreas): Unfortunately we can't use TypedComponentFallbackProvider here because it may not be implemented for all components of interest.
    // This sadly means that there's a bit of unnecessary back and forth between arrow array and untyped that could be avoided otherwise.
    pub fn component_or_fallback<C: re_types::Component>(
        &self,
        ctx: &ViewerContext<'_>,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &dyn re_viewer_context::ViewState,
        component_descr: &ComponentDescriptor,
    ) -> Result<C, ViewPropertyQueryError> {
        self.component_array_or_fallback::<C>(ctx, fallback_provider, view_state, component_descr)?
            .into_iter()
            .next()
            .ok_or(ComponentFallbackError::UnexpectedEmptyFallback.into())
    }

    // TODO(#6889): It's a bit sad that we can't derive the type of `C` from the `ComponentDescriptor`, which means
    //              that the call site has to specify the return type.
    /// Get the component array for a given type or its fallback if the component is not present or empty.
    pub fn component_array_or_fallback<C: re_types::Component>(
        &self,
        ctx: &ViewerContext<'_>,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &dyn re_viewer_context::ViewState,
        component_descr: &ComponentDescriptor,
    ) -> Result<Vec<C>, ViewPropertyQueryError> {
        C::from_arrow(
            self.component_or_fallback_raw(ctx, component_descr, fallback_provider, view_state)
                .as_ref(),
        )
        .map_err(|err| err.into())
    }

    /// Get a single component or None, not using any fallbacks.
    #[inline]
    pub fn component_or_empty<C: re_types::Component>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Result<Option<C>, DeserializationError> {
        self.component_array(component_descr)
            .map(|v| v.and_then(|v| v.into_iter().next()))
    }

    /// Get the component array for a given type, not using any fallbacks.
    pub fn component_array<C: re_types::Component>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Result<Option<Vec<C>>, DeserializationError> {
        self.component_raw(component_descr)
            .map(|raw| C::from_arrow(raw.as_ref()))
            .transpose()
    }

    /// Get the component array for a given type or an empty array, not using any fallbacks.
    pub fn component_array_or_empty<C: re_types::Component>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Result<Vec<C>, DeserializationError> {
        self.component_array(component_descr)
            .map(|value| value.unwrap_or_default())
    }

    pub fn component_row_id(&self, component_name: ComponentName) -> Option<re_chunk::RowId> {
        self.query_results
            .get_by_name(&component_name)
            .and_then(|unit| unit.row_id())
    }

    pub fn component_raw(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Option<arrow::array::ArrayRef> {
        self.query_results
            .get(component_descr)
            .and_then(|unit| unit.component_batch_raw(component_descr))
    }

    fn component_or_fallback_raw(
        &self,
        ctx: &ViewerContext<'_>,
        component_descr: &ComponentDescriptor,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &dyn re_viewer_context::ViewState,
    ) -> arrow::array::ArrayRef {
        if let Some(value) = self.component_raw(component_descr) {
            if value.len() > 0 {
                return value;
            }
        }
        fallback_provider.fallback_for(
            &self.query_context(ctx, view_state),
            component_descr.component_name,
        )
    }

    /// Save change to a blueprint component.
    pub fn save_blueprint_component(
        &self,
        ctx: &ViewerContext<'_>,
        components: &dyn ComponentBatch,
    ) {
        ctx.save_blueprint_component(&self.blueprint_store_path, components);
    }

    /// Clears a blueprint component.
    pub fn clear_blueprint_component<C: re_types::Component>(&self, ctx: &ViewerContext<'_>) {
        ctx.clear_blueprint_component_by_name(&self.blueprint_store_path, C::name());
    }

    /// Resets a blueprint component to the value it had in the default blueprint.
    pub fn reset_blueprint_component<C: re_types::Component>(&self, ctx: &ViewerContext<'_>) {
        ctx.reset_blueprint_component_by_name(&self.blueprint_store_path, C::name());
    }

    /// Resets all components to the values they had in the default blueprint.
    pub fn reset_all_components(&self, ctx: &ViewerContext<'_>) {
        // Don't use `self.query_results.components.keys()` since it may already have some components missing since they didn't show up in the query.
        for component_descr in &self.component_descrs {
            ctx.reset_blueprint_component_by_name(
                &self.blueprint_store_path,
                component_descr.component_name,
            );
        }
    }

    /// Resets all components to empty values, i.e. the fallback.
    pub fn reset_all_components_to_empty(&self, ctx: &ViewerContext<'_>) {
        for component_descr in self.query_results.components.keys() {
            ctx.clear_blueprint_component_by_name(
                &self.blueprint_store_path,
                component_descr.component_name,
            );
        }
    }

    /// Returns whether any property is non-empty.
    pub fn any_non_empty(&self) -> bool {
        self.query_results
            .components
            .keys()
            .any(|descr| self.component_raw(descr).is_some_and(|raw| !raw.is_empty()))
    }

    /// Create a query context for this view property.
    pub fn query_context<'a>(
        &'a self,
        viewer_ctx: &'a ViewerContext<'_>,
        view_state: &'a dyn re_viewer_context::ViewState,
    ) -> QueryContext<'a> {
        QueryContext {
            viewer_ctx,
            target_entity_path: &self.blueprint_store_path,
            archetype_name: Some(self.archetype_name),
            query: &self.blueprint_query,
            view_state,
            view_ctx: None,
        }
    }
}

/// Entity path in the blueprint store where all components of the given view property archetype are
/// stored.
pub fn entity_path_for_view_property(
    view_id: ViewId,
    _blueprint_entity_tree: &EntityTree,
    archetype_name: ArchetypeName,
) -> EntityPath {
    // TODO(andreas,jleibs):
    // We want to search the subtree for occurrences of the property archetype here.
    // Only if none is found we make up a new (standardized) path.
    // There's some nuances to figure out what happens when we find the archetype several times.
    // Also, we need to specify what it means to "find" the archetype (likely just matching the indicator?).
    let view_blueprint_path = view_id.as_entity_path();

    // Use short_name instead of full_name since full_name has dots and looks too much like an indicator component.
    view_blueprint_path.join(&EntityPath::from_single_string(archetype_name.short_name()))
}
