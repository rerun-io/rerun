use re_log_types::StoreId;
use re_redap_browser::RedapServers;
use re_viewer_context::{EntryKind, Route};

use crate::history::History;

/// Where the user is, and where they have been.
#[derive(Default)]
pub(crate) struct NavigationState {
    /// A stack of [`Route`]s that represents tab-like navigation of the user.
    pub routes: Navigation,

    /// A history of urls the viewer has visited.
    ///
    /// This is not updated if this is a web viewer with control over web history.
    pub history: History,
}

impl NavigationState {
    /// The active recording [`StoreId`], if any, derived from the current [`Route`].
    pub fn active_recording_id(&self) -> Option<&StoreId> {
        self.routes.current().recording_id()
    }

    /// Makes the entry kind of the current route agree with the catalog, once the catalog knows.
    pub fn resolve_route(&mut self, redap_servers: &RedapServers) {
        let Route::RedapEntry {
            origin,
            entry_id,
            kind,
        } = self.routes.current()
        else {
            return;
        };

        let (origin, entry_id, kind) = (origin.clone(), *entry_id, *kind);

        let resolved_kind = resolve_entry_kind(kind, redap_servers.entry_kind(&origin, entry_id));

        if resolved_kind != kind {
            self.routes.replace(Route::RedapEntry {
                origin,
                entry_id,
                kind: resolved_kind,
            });
        }
    }

    /// Whether the current route may still change as data or metadata loads.
    pub fn is_resolving_route(&self, redap_servers: &RedapServers) -> bool {
        match self.routes.current() {
            Route::Loading(_) => true,
            Route::RedapEntry {
                origin,
                entry_id,
                kind,
            } => {
                redap_servers.is_entry_kind_pending(origin, *entry_id)
                    || resolve_entry_kind(*kind, redap_servers.entry_kind(origin, *entry_id))
                        != *kind
            }
            _ => false,
        }
    }
}

/// The entry kind a route should carry, given what it has and what the catalog says.
fn resolve_entry_kind(
    route_kind: Option<EntryKind>,
    catalog_kind: Option<EntryKind>,
) -> Option<EntryKind> {
    match (catalog_kind, route_kind) {
        (Some(EntryKind::Dataset(_)), Some(dataset @ EntryKind::Dataset(_))) => Some(dataset),
        (Some(catalog_kind), _) => Some(catalog_kind),
        (None, route_kind) => route_kind,
    }
}

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
    use std::assert_matches;

    use std::path::PathBuf;

    use re_log_channel::LogSource;
    use re_log_types::EntryId;
    use re_viewer_context::StoreHub;
    use re_viewer_context::viewer_url::ViewerUrl;

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
        assert_matches!(navigation.current(), Route::Loading(_));
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

    /// A url can call an entry a dataset when the catalog knows it to be a table. The route then
    /// shows a table, and the url it shares is an entry url.
    #[test]
    fn a_table_behind_a_dataset_url_shares_an_entry_url() {
        let origin: re_uri::Origin = "rerun://localhost:51234".parse().expect("valid origin");
        let entry_id = EntryId::new();

        let kind = resolve_entry_kind(
            Some(EntryKind::Dataset(re_uri::DatasetResource::default())),
            Some(EntryKind::Table),
        );
        assert_eq!(kind, Some(EntryKind::Table));

        let url = ViewerUrl::from_route(
            &StoreHub::test_hub(),
            &Route::RedapEntry {
                origin: origin.clone(),
                entry_id,
                kind,
            },
        )
        .expect("a redap entry route has a url");

        assert_eq!(
            url,
            ViewerUrl::RedapEntry(re_uri::EntryUri::new(origin, entry_id))
        );
    }

    /// The catalog says nothing about which resource of a dataset is on display, so a route that
    /// already shows one keeps it. A route that shows a table instead falls back to the default
    /// resource.
    #[test]
    fn a_dataset_route_keeps_its_resource() {
        let catalog_kind = Some(EntryKind::Dataset(re_uri::DatasetResource::default()));

        assert_eq!(
            resolve_entry_kind(
                Some(EntryKind::Dataset(re_uri::DatasetResource::Assets)),
                catalog_kind
            ),
            Some(EntryKind::Dataset(re_uri::DatasetResource::Assets))
        );
        assert_eq!(
            resolve_entry_kind(Some(EntryKind::Table), catalog_kind),
            catalog_kind
        );
    }

    /// A route that names no kind takes the one from the catalog, and a catalog that knows nothing
    /// leaves the route as it is.
    #[test]
    fn an_unknown_kind_follows_the_catalog() {
        assert_eq!(
            resolve_entry_kind(None, Some(EntryKind::Table)),
            Some(EntryKind::Table)
        );
        assert_eq!(resolve_entry_kind(None, None), None);
        assert_eq!(
            resolve_entry_kind(Some(EntryKind::Table), None),
            Some(EntryKind::Table)
        );
    }
}
