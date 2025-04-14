use re_viewer_context::DisplayMode;

// TODO(grtlr): Move this to `history.rs` and merge with the router there.

/// The navigation history of the viewer.
///
/// This object should never be exposed to directly via contexts. Instead,
/// we retrieve the display mode and pass that around.
#[derive(Default)]
pub(crate) struct Navigation(Vec<DisplayMode>);

impl Navigation {
    pub fn push_unique(&mut self, display_mode: DisplayMode) {
        let before = self.0.len();
        self.0.retain(|d| d != &display_mode);
        let after = self.0.len();

        if before == after {
            re_log::debug!("Pushed new display mode `{:?}`", display_mode);
        } else {
            re_log::debug!("Reusing existing display mode `{:?}`", display_mode);
        }
        self.0.push(display_mode);
    }

    pub fn pop(&mut self) -> Option<DisplayMode> {
        let previous = self.0.pop();
        re_log::debug!("Popped display mode `{:?}`", previous);
        previous
    }

    pub fn peek(&self) -> &DisplayMode {
        self.0.last().unwrap_or(&DisplayMode::WelcomeScreen)
    }
}
