use re_types::{
    Archetype as _,
    blueprint::{
        archetypes,
        components::{Enabled, ForceDistance, ForceIterations, ForceStrength, VisualBounds2D},
    },
    components::Position2D,
};
use re_viewer_context::{TypedComponentFallbackProvider, ViewStateExt as _};

use crate::{GraphView, ui::GraphViewState};

fn valid_bound(rect: &egui::Rect) -> bool {
    rect.is_finite() && rect.is_positive()
}

impl TypedComponentFallbackProvider<VisualBounds2D> for GraphView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> VisualBounds2D {
        let Ok(state) = ctx.view_state().downcast_ref::<GraphViewState>() else {
            return VisualBounds2D::default();
        };

        match state.layout_state.bounding_rect() {
            Some(rect) if valid_bound(&rect) => rect.into(),
            _ => VisualBounds2D::default(),
        }
    }
}

impl TypedComponentFallbackProvider<Enabled> for GraphView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Enabled {
        match ctx.archetype_name {
            Some(name) if name == archetypes::ForceLink::name() => true.into(),
            Some(name) if name == archetypes::ForceManyBody::name() => true.into(),
            Some(name) if name == archetypes::ForcePosition::name() => true.into(),
            _ => false.into(),
        }
    }
}

impl TypedComponentFallbackProvider<ForceDistance> for GraphView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> ForceDistance {
        (60.).into()
    }
}

impl TypedComponentFallbackProvider<ForceStrength> for GraphView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> ForceStrength {
        match ctx.archetype_name {
            Some(name) if name == archetypes::ForceManyBody::name() => (-60.0).into(),
            Some(name) if name == archetypes::ForcePosition::name() => (0.01).into(),
            _ => (1.0).into(),
        }
    }
}

impl TypedComponentFallbackProvider<ForceIterations> for GraphView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> ForceIterations {
        match ctx.archetype_name {
            Some(name) if name == archetypes::ForceLink::name() => 3.into(),
            Some(name) if name == archetypes::ForceCollisionRadius::name() => 1.into(),
            _ => Default::default(),
        }
    }
}

impl TypedComponentFallbackProvider<Position2D> for GraphView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Position2D {
        Default::default()
    }
}

re_viewer_context::impl_component_fallback_provider!(GraphView => [VisualBounds2D, Enabled, ForceDistance, ForceStrength, ForceIterations, Position2D]);
