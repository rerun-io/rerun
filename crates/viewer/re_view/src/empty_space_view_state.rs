use re_viewer_context::SpaceViewState;

/// View state without any contents.
#[derive(Default)]
pub struct EmptySpaceViewState;

impl SpaceViewState for EmptySpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
