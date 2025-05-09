use re_types::{
    Archetype, Component, ComponentDescriptor,
    blueprint::{
        archetypes::{ForceCenter, ForceCollisionRadius, ForceLink, ForceManyBody, ForcePosition},
        components::{Enabled, ForceDistance, ForceIterations, ForceStrength},
    },
    components::Position2D,
};
use re_viewer_context::{ComponentFallbackProvider, ViewQuery, ViewState, ViewerContext};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

#[derive(Debug, PartialEq)]
pub struct ForceLayoutParams {
    // Link
    pub(super) force_link_enabled: Enabled,
    pub(super) force_link_distance: ForceDistance,
    pub(super) force_link_iterations: ForceIterations,
    // Many body
    pub(super) force_many_body_enabled: Enabled,
    pub(super) force_many_body_strength: ForceStrength,
    // Position
    pub(super) force_position_enabled: Enabled,
    pub(super) force_position_strength: ForceStrength,
    pub(super) force_position_pos: Position2D,
    // Center
    pub(super) force_center_enabled: Enabled,
    pub(super) force_center_strength: ForceStrength,
    // Collision
    pub(super) force_collision_enabled: Enabled,
    pub(super) force_collision_strength: ForceStrength,
    pub(super) force_collision_iterations: ForceIterations,
}

/// Convenience struct for querying the components of a blueprint archetype or its fallbacks.
struct QueryArchetype<'a, T> {
    ctx: &'a ViewerContext<'a>,
    provider: &'a dyn ComponentFallbackProvider,
    view_state: &'a dyn ViewState,
    property: ViewProperty,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: Archetype> QueryArchetype<'a, T> {
    fn new(
        ctx: &'a ViewerContext<'a>,
        query: &'a ViewQuery<'a>,
        provider: &'a dyn ComponentFallbackProvider,
        view_state: &'a dyn ViewState,
    ) -> Self {
        let property = ViewProperty::from_archetype::<T>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        Self {
            ctx,
            provider,
            view_state,
            property,
            _marker: Default::default(),
        }
    }

    fn get<R>(&self, component_descr: &ComponentDescriptor) -> Result<R, ViewPropertyQueryError>
    where
        R: Component + Default,
    {
        self.property.component_or_fallback(
            self.ctx,
            self.provider,
            self.view_state,
            component_descr,
        )
    }
}

impl ForceLayoutParams {
    pub fn get(
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        provider: &dyn ComponentFallbackProvider,
        view_state: &dyn ViewState,
    ) -> Result<Self, ViewPropertyQueryError> {
        // Query the components for the archetype
        let force_link = QueryArchetype::<ForceLink>::new(ctx, query, provider, view_state);
        let force_many = QueryArchetype::<ForceManyBody>::new(ctx, query, provider, view_state);
        let force_position = QueryArchetype::<ForcePosition>::new(ctx, query, provider, view_state);
        let force_center = QueryArchetype::<ForceCenter>::new(ctx, query, provider, view_state);
        let force_collision =
            QueryArchetype::<ForceCollisionRadius>::new(ctx, query, provider, view_state);

        Ok(Self {
            // Link
            force_link_enabled: force_link.get(&ForceLink::descriptor_enabled())?,
            force_link_distance: force_link.get(&ForceLink::descriptor_distance())?,
            force_link_iterations: force_link.get(&ForceLink::descriptor_iterations())?,
            // Many body
            force_many_body_enabled: force_many.get(&ForceManyBody::descriptor_enabled())?,
            force_many_body_strength: force_many.get(&ForceManyBody::descriptor_strength())?,
            // Position
            force_position_enabled: force_position.get(&ForcePosition::descriptor_enabled())?,
            force_position_strength: force_position.get(&ForcePosition::descriptor_strength())?,
            force_position_pos: force_position.get(&ForcePosition::descriptor_position())?,
            // Center
            force_center_enabled: force_center.get(&ForceCenter::descriptor_enabled())?,
            force_center_strength: force_center.get(&ForceCenter::descriptor_strength())?,
            // Collision
            force_collision_enabled: force_collision
                .get(&ForceCollisionRadius::descriptor_enabled())?,
            force_collision_strength: force_collision
                .get(&ForceCollisionRadius::descriptor_strength())?,
            force_collision_iterations: force_collision
                .get(&ForceCollisionRadius::descriptor_iterations())?,
        })
    }
}
