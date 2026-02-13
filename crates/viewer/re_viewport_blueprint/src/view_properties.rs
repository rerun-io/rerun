use arrow::array::ArrayRef;
use re_chunk::{ComponentIdentifier, ComponentType};
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_entity_db::external::re_query::LatestAtResults;
use re_log_types::EntityPath;
use re_sdk_types::{
    Archetype, ArchetypeName, ComponentBatch, ComponentDescriptor, DeserializationError,
};
use re_viewer_context::external::re_entity_db::EntityTree;
use re_viewer_context::{
    BlueprintContext, ComponentFallbackError, QueryContext, ViewContext, ViewId,
    ViewSystemExecutionError, ViewerContext,
};

#[derive(thiserror::Error, Debug)]
pub enum ViewPropertyQueryError {
    #[error(transparent)]
    DeserializationError(#[from] re_sdk_types::DeserializationError),

    #[error(transparent)]
    ComponentFallbackError(#[from] ComponentFallbackError),
}

impl From<ViewPropertyQueryError> for ViewSystemExecutionError {
    fn from(val: ViewPropertyQueryError) -> Self {
        match val {
            ViewPropertyQueryError::DeserializationError(err) => err.into(),
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
        re_tracing::profile_function!(A::name());

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
            component_descrs.iter().map(|desc| desc.component),
        );

        Self {
            blueprint_store_path,
            archetype_name,
            component_descrs,
            query_results,
            blueprint_query,
        }
    }

    /// Get the value of a specific component or its fallback if the component is not present.
    pub fn component_or_fallback<C: re_sdk_types::Component>(
        &self,
        ctx: &ViewContext<'_>,
        component: ComponentIdentifier,
    ) -> Result<C, ViewPropertyQueryError> {
        re_tracing::profile_function!(component);

        self.component_array_or_fallback::<C>(ctx, component)?
            .into_iter()
            .next()
            .ok_or_else(|| ComponentFallbackError::UnexpectedEmptyFallback.into())
    }

    /// Get the component array for a given type or its fallback if the component is not present or empty.
    pub fn component_array_or_fallback<C: re_sdk_types::Component>(
        &self,
        ctx: &ViewContext<'_>,
        component: ComponentIdentifier,
    ) -> Result<Vec<C>, ViewPropertyQueryError> {
        C::from_arrow(
            self.component_or_fallback_raw(ctx, component, Some(C::name()))
                .as_ref(),
        )
        .map_err(|err| err.into())
    }

    /// Get a single component or None, not using any fallbacks.
    #[inline]
    pub fn component_or_empty<C: re_sdk_types::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Result<Option<C>, DeserializationError> {
        self.component_array(component)
            .map(|v| v?.into_iter().next())
    }

    /// Get the component array for a given type, not using any fallbacks.
    pub fn component_array<C: re_sdk_types::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Result<Option<Vec<C>>, DeserializationError> {
        self.component_raw(component)
            .map(|raw| C::from_arrow(raw.as_ref()))
            .transpose()
    }

    /// Get the component array for a given type or an empty array, not using any fallbacks.
    pub fn component_array_or_empty<C: re_sdk_types::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Result<Vec<C>, DeserializationError> {
        self.component_array(component)
            .map(|value| value.unwrap_or_default())
    }

    pub fn component_row_id(&self, component: ComponentIdentifier) -> Option<re_chunk::RowId> {
        self.query_results.get(component)?.row_id()
    }

    pub fn component_raw(&self, component: ComponentIdentifier) -> Option<arrow::array::ArrayRef> {
        self.query_results
            .get(component)?
            .component_batch_raw(component)
    }

    fn component_or_fallback_raw(
        &self,
        ctx: &ViewContext<'_>,
        component_identifier: ComponentIdentifier,
        component_type: Option<ComponentType>,
    ) -> ArrayRef {
        if let Some(value) = self.component_raw(component_identifier)
            && !value.is_empty()
        {
            return value;
        }

        ctx.viewer_ctx.component_fallback_registry.fallback_for(
            component_identifier,
            component_type,
            &self.query_context(ctx),
        )
    }

    /// Save change to a blueprint component.
    pub fn save_blueprint_component(
        &self,
        ctx: &impl BlueprintContext,
        component_descr: &ComponentDescriptor,
        component_batch: &dyn ComponentBatch,
    ) {
        if !self.component_descrs.contains(component_descr) {
            #[expect(clippy::panic)] // Debug only.
            if cfg!(debug_assertions) {
                panic!(
                    "trying to save a blueprint component `{component_descr}` that is not part of the view property for archetype `{}`",
                    self.archetype_name
                );
            } else {
                re_log::warn_once!(
                    "trying to save a blueprint component `{component_descr}` that is not part of the view property for archetype `{}`",
                    self.archetype_name
                );
            }
        }
        ctx.save_blueprint_component(
            self.blueprint_store_path.clone(),
            component_descr,
            component_batch,
        );
    }

    /// Clears a blueprint component.
    pub fn clear_blueprint_component(
        &self,
        ctx: &ViewerContext<'_>,
        component_descr: ComponentDescriptor,
    ) {
        ctx.clear_blueprint_component(self.blueprint_store_path.clone(), component_descr);
    }

    /// Resets a blueprint component to the value it had in the default blueprint.
    pub fn reset_blueprint_component(
        &self,
        ctx: &ViewerContext<'_>,
        component_descr: ComponentDescriptor,
    ) {
        ctx.reset_blueprint_component(self.blueprint_store_path.clone(), component_descr);
    }

    /// Resets all components to the values they had in the default blueprint.
    pub fn reset_all_components(&self, ctx: &ViewerContext<'_>) {
        // Don't use `self.query_results.components.keys()` since it may already have some components missing since they didn't show up in the query.
        for component_descr in self.component_descrs.iter().cloned() {
            ctx.reset_blueprint_component(self.blueprint_store_path.clone(), component_descr);
        }
    }

    /// Resets all components to empty values, i.e. the fallback.
    pub fn reset_all_components_to_empty(&self, ctx: &ViewerContext<'_>) {
        let blueprint_storage_engine = ctx.blueprint_db().storage_engine();
        let blueprint_store = blueprint_storage_engine.store();
        for component in self.query_results.components.keys().copied() {
            if let Some(component_descr) =
                blueprint_store.entity_component_descriptor(&self.blueprint_store_path, component)
            {
                ctx.clear_blueprint_component(self.blueprint_store_path.clone(), component_descr);
            }
        }
    }

    /// Returns whether any property is non-empty.
    pub fn any_non_empty(&self) -> bool {
        self.query_results.components.keys().any(|component| {
            self.component_raw(*component)
                .is_some_and(|raw| !raw.is_empty())
        })
    }

    /// Create a query context for this view property.
    pub fn query_context<'a>(&'a self, view_ctx: &'a ViewContext<'_>) -> QueryContext<'a> {
        QueryContext {
            view_ctx,
            target_entity_path: &self.blueprint_store_path,
            instruction_id: None,
            archetype_name: Some(self.archetype_name),
            query: self.blueprint_query.clone(),
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
    // Also, we need to specify what it means to "find" the archetype.
    let view_blueprint_path = view_id.as_entity_path();

    // Use short_name instead of full_name since full_name has dots.
    view_blueprint_path.join(&EntityPath::from_single_string(archetype_name.short_name()))
}
