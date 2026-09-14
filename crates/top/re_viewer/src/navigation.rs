use re_viewer_context::Route;

/// Keeps track of the current [`Route`] of the viewer.
pub(crate) struct Navigation {
    current_mode: Route,
    start_mode: Route,

    /// The route hidden by the current loading screen.
    ///
    /// Loading is transient UI state and must not replace the route that history tracks.
    loading_return_mode: Option<Route>,
}

impl Default for Navigation {
    fn default() -> Self {
        let start_mode = Route::welcome_page();
        Self {
            current_mode: start_mode.clone(),
            start_mode,
            loading_return_mode: None,
        }
    }
}

impl Navigation {
    /// Resets to use the start route, which is also the fallback mode for navigation.
    pub fn reset(&mut self) {
        self.current_mode = self.start_mode.clone();
        self.loading_return_mode = None;
    }

    pub fn replace(&mut self, new_mode: Route) -> Route {
        if matches!(new_mode, Route::Loading(_)) {
            if !matches!(self.current_mode, Route::Loading(_)) {
                self.loading_return_mode = Some(self.current_mode.clone());
            }
        } else {
            self.loading_return_mode = None;
        }

        let previous = std::mem::replace(&mut self.current_mode, new_mode);

        if previous != *self.current() {
            re_log::trace!("Navigated from {previous:?} to {:?}", self.current());
        }

        previous
    }

    /// Restores the route hidden by the current loading screen.
    ///
    /// Does nothing if there is no loading screen active.
    pub fn return_from_loading(&mut self) {
        let Some(return_mode) = self.loading_return_mode.take() else {
            return;
        };

        let previous = std::mem::replace(&mut self.current_mode, return_mode);
        re_log::trace!("Navigated from {previous:?} to {:?}", self.current());
    }

    /// Current visible state.
    pub fn current(&self) -> &Route {
        &self.current_mode
    }

    /// Current stable route, excluding a transient loading screen.
    pub fn history_route(&self) -> &Route {
        self.loading_return_mode
            .as_ref()
            .unwrap_or(&self.current_mode)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use re_log_channel::LogSource;

    use super::*;

    fn loading_route(path: &str) -> Route {
        Route::Loading(Box::new(LogSource::File {
            path: PathBuf::from(path),
        }))
    }

    #[test]
    fn loading_is_layered_over_the_stable_route() {
        let mut navigation = Navigation::default();
        let stable_route = Route::Settings {
            return_route: Box::new(Route::welcome_page()),
        };
        navigation.replace(stable_route.clone());

        navigation.replace(loading_route("first.rrd"));
        assert!(matches!(navigation.current(), Route::Loading(_)));
        assert_eq!(navigation.history_route(), &stable_route);

        navigation.replace(loading_route("second.rrd"));
        assert_eq!(navigation.history_route(), &stable_route);

        navigation.return_from_loading();
        assert_eq!(navigation.current(), &stable_route);
        assert_eq!(navigation.history_route(), &stable_route);
    }

    #[test]
    fn successful_navigation_replaces_the_loading_layer() {
        let mut navigation = Navigation::default();
        navigation.replace(loading_route("recording.rrd"));

        let destination = Route::Settings {
            return_route: Box::new(Route::welcome_page()),
        };
        navigation.replace(destination.clone());

        assert_eq!(navigation.current(), &destination);
        assert_eq!(navigation.history_route(), &destination);
    }
}
