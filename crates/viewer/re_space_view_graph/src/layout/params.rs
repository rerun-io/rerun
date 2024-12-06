use re_types::blueprint::{
    archetypes::ForceLink,
    components::{Enabled, ForceDistance},
};
use re_viewer_context::{ComponentFallbackProvider, SpaceViewState, ViewQuery, ViewerContext};
use re_viewport_blueprint::{ViewProperty, ViewPropertyQueryError};

#[derive(Debug)]
pub struct ForceLayoutParams {
    pub(super) force_link_enabled: Enabled,
    pub(super) force_link_distance: ForceDistance,
}

impl ForceLayoutParams {
    pub fn get(
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        fallback_provider: &dyn ComponentFallbackProvider,
        view_state: &dyn SpaceViewState,
    ) -> Result<Self, ViewPropertyQueryError> {
        let force_link_property = ViewProperty::from_archetype::<ForceLink>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );

        Ok(Self {
            force_link_enabled: force_link_property.component_or_fallback(
                ctx,
                fallback_provider,
                view_state,
            )?,
            force_link_distance: force_link_property.component_or_fallback(
                ctx,
                fallback_provider,
                view_state,
            )?,
        })
    }
}
