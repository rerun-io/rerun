use re_types::ComponentNameSet;
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
};

/// An empty system to accept all entities in the space view
#[derive(Default)]
pub struct EmptySystem {}

impl IdentifiedViewSystem for EmptySystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Empty".into()
    }
}

impl ViewPartSystem for EmptySystem {
    fn required_components(&self) -> ComponentNameSet {
        std::iter::empty().collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::empty().collect()
    }

    fn execute(
        &mut self,
        _ctx: &ViewerContext<'_>,
        _query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
