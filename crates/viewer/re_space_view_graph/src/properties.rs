use re_types::{
    blueprint::{
        archetypes,
        components::{Enabled, ForceDistance, VisualBounds2D},
    },
    Archetype as _,
};
use re_viewer_context::{SpaceViewStateExt as _, TypedComponentFallbackProvider};

use crate::{ui::GraphSpaceViewState, GraphSpaceView};

fn valid_bound(rect: &egui::Rect) -> bool {
    rect.is_finite() && rect.is_positive()
}

impl TypedComponentFallbackProvider<VisualBounds2D> for GraphSpaceView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> VisualBounds2D {
        let Ok(state) = ctx.view_state.downcast_ref::<GraphSpaceViewState>() else {
            return VisualBounds2D::default();
        };

        match state.layout_state.bounding_rect() {
            Some(rect) if valid_bound(&rect) => rect.into(),
            _ => VisualBounds2D::default(),
        }
    }
}

impl TypedComponentFallbackProvider<Enabled> for GraphSpaceView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Enabled {
        match ctx.archetype_name {
            Some(name) if name == archetypes::ForceLink::name() => true.into(),
            _ => false.into(),
        }
    }
}

impl TypedComponentFallbackProvider<ForceDistance> for GraphSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> ForceDistance {
        (50.).into()
    }
}

re_viewer_context::impl_component_fallback_provider!(GraphSpaceView => [VisualBounds2D, Enabled, ForceDistance]);
