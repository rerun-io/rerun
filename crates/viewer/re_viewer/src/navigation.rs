use re_viewer_context::DisplayMode;

// TODO(grtlr): Move this to `history.rs` and merge with the router there.

/// The navigation history of the viewer.
///
/// This object should never be exposed to directly via contexts. Instead,
/// we retrieve the display mode and pass that around.
#[derive(Default)]
pub(crate) struct Navigation(Vec<DisplayMode>);

// TODO(grtlr): Make the welcome screen a display mode too, and store its state.

impl Navigation {
    pub fn push(&mut self, display_mode: DisplayMode) {
        self.0.push(display_mode);
    }

    pub fn replace(&mut self, display_mode: DisplayMode) -> Option<DisplayMode> {
        let previous = self.0.pop();
        self.0.push(display_mode);
        previous
    }

    pub fn pop(&mut self) -> Option<DisplayMode> {
        self.0.pop()
    }

    pub fn peek(&self) -> &DisplayMode {
        self.0.first().unwrap_or(&DisplayMode::WelcomeScreen)
    }
}
