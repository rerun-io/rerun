use re_chunk::ComponentIdentifier;
use re_sdk_types::blueprint::archetypes::{
    ForceCenter, ForceCollisionRadius, ForceLink, ForceManyBody, ForcePosition,
};
use re_sdk_types::blueprint::components::{Enabled, ForceDistance, ForceIterations, ForceStrength};
use re_sdk_types::components::Position2D;
use re_sdk_types::{Archetype, Component};
use re_viewer_context::ViewContext;
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
    ctx: &'a ViewContext<'a>,
    property: ViewProperty,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: Archetype> QueryArchetype<'a, T> {
    fn new(ctx: &'a ViewContext<'a>) -> Self {
        let property = ViewProperty::from_archetype::<T>(
            ctx.viewer_ctx.blueprint_db(),
            ctx.blueprint_query(),
            ctx.view_id,
        );
        Self {
            ctx,
            property,
            _marker: Default::default(),
        }
    }

    fn get<R>(&self, component: ComponentIdentifier) -> Result<R, ViewPropertyQueryError>
    where
        R: Component + Default,
    {
        self.property.component_or_fallback(self.ctx, component)
    }
}

impl ForceLayoutParams {
    pub fn get(ctx: &ViewContext<'_>) -> Result<Self, ViewPropertyQueryError> {
        // Query the components for the archetype
        let force_link = QueryArchetype::<ForceLink>::new(ctx);
        let force_many = QueryArchetype::<ForceManyBody>::new(ctx);
        let force_position = QueryArchetype::<ForcePosition>::new(ctx);
        let force_center = QueryArchetype::<ForceCenter>::new(ctx);
        let force_collision = QueryArchetype::<ForceCollisionRadius>::new(ctx);

        Ok(Self {
            // Link
            force_link_enabled: force_link.get(ForceLink::descriptor_enabled().component)?,
            force_link_distance: force_link.get(ForceLink::descriptor_distance().component)?,
            force_link_iterations: force_link.get(ForceLink::descriptor_iterations().component)?,
            // Many body
            force_many_body_enabled: force_many
                .get(ForceManyBody::descriptor_enabled().component)?,
            force_many_body_strength: force_many
                .get(ForceManyBody::descriptor_strength().component)?,
            // Position
            force_position_enabled: force_position
                .get(ForcePosition::descriptor_enabled().component)?,
            force_position_strength: force_position
                .get(ForcePosition::descriptor_strength().component)?,
            force_position_pos: force_position
                .get(ForcePosition::descriptor_position().component)?,
            // Center
            force_center_enabled: force_center.get(ForceCenter::descriptor_enabled().component)?,
            force_center_strength: force_center
                .get(ForceCenter::descriptor_strength().component)?,
            // Collision
            force_collision_enabled: force_collision
                .get(ForceCollisionRadius::descriptor_enabled().component)?,
            force_collision_strength: force_collision
                .get(ForceCollisionRadius::descriptor_strength().component)?,
            force_collision_iterations: force_collision
                .get(ForceCollisionRadius::descriptor_iterations().component)?,
        })
    }
}
