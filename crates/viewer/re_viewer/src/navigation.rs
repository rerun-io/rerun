use re_viewer_context::Route;

/// Keeps track of the current [`Route`] of the viewer.
pub(crate) struct Navigation {
    current_mode: Route,
    start_mode: Route,
}

impl Default for Navigation {
    fn default() -> Self {
        let start_mode = Route::welcome_page();
        Self {
            current_mode: start_mode.clone(),
            start_mode,
        }
    }
}

impl Navigation {
    /// Resets to use the start route, which is also the fallback mode for
    /// navigation.
    ///
    /// This is defined in the default implementation for [`Navigation`]
    pub fn reset(&mut self) {
        self.current_mode = self.start_mode.clone();
    }

    pub fn replace(&mut self, new_mode: Route) -> Route {
        let previous = std::mem::replace(&mut self.current_mode, new_mode);

        if previous != *self.current() {
            re_log::trace!("Navigated from {previous:?} to {:?}", self.current());
        }

        previous
    }

    /// Current state
    pub fn current(&self) -> &Route {
        &self.current_mode
    }
}
