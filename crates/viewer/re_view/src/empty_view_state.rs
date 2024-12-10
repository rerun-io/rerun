use re_viewer_context::ViewState;

/// View state without any contents.
#[derive(Default)]
pub struct EmptyViewState;

impl ViewState for EmptyViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
