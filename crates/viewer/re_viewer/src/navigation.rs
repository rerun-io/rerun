use re_viewer_context::DisplayMode;

/// Keeps track of the current display mode of the viewer.
pub(crate) struct Navigation {
    current_mode: DisplayMode,
    start_mode: DisplayMode,
}

impl Default for Navigation {
    fn default() -> Self {
        let start_mode = DisplayMode::welcome_page();
        Self {
            current_mode: start_mode.clone(),
            start_mode,
        }
    }
}

impl Navigation {
    /// Resets to use the start display mode, which is also the fallback mode for
    /// navigation.
    ///
    /// This is defined in the default implementation for [`Navigation`]
    pub fn reset(&mut self) {
        self.current_mode = self.start_mode.clone();
    }

    pub fn replace(&mut self, new_mode: DisplayMode) -> DisplayMode {
        let previous = std::mem::replace(&mut self.current_mode, new_mode);

        if previous != *self.current() {
            re_log::trace!("Navigated from {previous:?} to {:?}", self.current());
        }

        previous
    }

    /// Current state
    pub fn current(&self) -> &DisplayMode {
        &self.current_mode
    }
}
