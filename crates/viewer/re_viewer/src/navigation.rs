use re_viewer_context::DisplayMode;

/// The navigation history of the viewer.
///
/// This object should never be exposed to directly via contexts. Instead,
/// we retrieve the display mode and pass that around.
pub(crate) struct Navigation {
    history: Vec<DisplayMode>,
    default: DisplayMode,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            history: Default::default(),
            default: DisplayMode::RedapServer(re_redap_browser::EXAMPLES_ORIGIN.clone()),
        }
    }
}

impl Navigation {
    // TODO(grtlr): In the future we should have something like `push_unique`, but for
    // this we first need all display modes to contain more information.

    pub fn push(&mut self, display_mode: DisplayMode) {
        re_log::debug!("Pushed display mode `{:?}`", display_mode);
        self.history.push(display_mode);
    }

    pub fn replace(&mut self, display_mode: DisplayMode) -> Option<DisplayMode> {
        let previous = self.history.pop();
        re_log::debug!("Replaced `{:?}` with `{:?}`", previous, display_mode);
        self.history.push(display_mode);
        previous
    }

    pub fn pop(&mut self) -> Option<DisplayMode> {
        let previous = self.history.pop();
        re_log::debug!("Popped display mode `{:?}`", previous);
        previous
    }

    pub fn peek(&self) -> &DisplayMode {
        self.history.last().unwrap_or(&self.default)
    }
}
