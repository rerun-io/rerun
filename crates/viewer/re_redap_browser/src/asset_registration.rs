//! The asset registrations a server is working on, the ones it refused, and the assets it was
//! asked to drop.

use re_log_types::EntryId;
use re_log_types::external::re_types_core::SegmentId;
use re_redap_client::{ApiError, AssetRegistrationError};

/// An asset the user asked a server to register.
pub struct AssetRegistration {
    /// The dataset the asset is registered with.
    pub dataset_id: EntryId,

    /// Where the asset is read from, as typed by the user.
    ///
    /// A dataset registers one source uri at a time, so this identifies the registration.
    pub source_uri: String,

    pub state: RegistrationState,
}

impl AssetRegistration {
    /// The asset that stays registered after the server refused the registration, if it got that
    /// far.
    ///
    /// A server that refuses a source before it starts registering registers nothing, so there is
    /// nothing to clean up.
    pub fn failed_asset(&self) -> Option<&SegmentId> {
        match &self.state {
            RegistrationState::Pending | RegistrationState::Registered => None,
            RegistrationState::Failed(err) => err.asset_id.as_ref(),
        }
    }
}

pub enum RegistrationState {
    /// The server is still working on it.
    Pending,

    /// The server took the asset, and the dataset's asset list has yet to show it.
    Registered,

    /// The server refused it, along with the asset that stays registered if it got that far.
    Failed(AssetRegistrationError),
}

/// The registrations started for one server's datasets, oldest first.
///
/// A registration is dropped once the dataset's asset list shows the asset, so it stays listed
/// across the refetch that follows the server taking it. A failed one is kept until it is
/// dismissed.
#[derive(Default)]
pub struct AssetRegistrations(Vec<AssetRegistration>);

impl AssetRegistrations {
    /// Lists a registration the server has been asked for.
    ///
    /// Registering a source uri again takes over its registration, so a failed one can be retried.
    pub fn start(&mut self, dataset_id: EntryId, source_uri: String) {
        let registration = AssetRegistration {
            dataset_id,
            source_uri,
            state: RegistrationState::Pending,
        };

        match self.find(dataset_id, &registration.source_uri) {
            Some(index) => self.0[index] = registration,
            None => self.0.push(registration),
        }
    }

    /// Records the server's answer.
    ///
    /// A registration the server took keeps its slot until [`Self::dismiss`] drops it, which is
    /// once the dataset's asset list shows the asset.
    pub fn finish(
        &mut self,
        dataset_id: EntryId,
        source_uri: &str,
        result: Result<(), AssetRegistrationError>,
    ) {
        let Some(index) = self.find(dataset_id, source_uri) else {
            return;
        };

        self.0[index].state = match result {
            Ok(()) => RegistrationState::Registered,
            Err(err) => RegistrationState::Failed(err),
        };
    }

    pub fn dismiss(&mut self, dataset_id: EntryId, source_uri: &str) {
        if let Some(index) = self.find(dataset_id, source_uri) {
            self.0.remove(index);
        }
    }

    /// Stops listing the registrations the server took whose asset the list now shows.
    ///
    /// `list_caught_up` says whether a dataset's asset list is done catching up with the server.
    pub fn clear_registered(&mut self, list_caught_up: impl Fn(EntryId) -> bool) {
        self.0.retain(|registration| {
            !matches!(registration.state, RegistrationState::Registered)
                || !list_caught_up(registration.dataset_id)
        });
    }

    /// The datasets whose asset list has yet to show an asset the server registered.
    ///
    /// A dataset appears once per such registration.
    pub fn datasets_waiting_for_list(&self) -> impl Iterator<Item = EntryId> {
        self.0
            .iter()
            .filter(|registration| matches!(registration.state, RegistrationState::Registered))
            .map(|registration| registration.dataset_id)
    }

    /// Stops listing the registration that failed with this asset still registered.
    ///
    /// The asset has just been unregistered, so the failure is cleaned up and the registration
    /// stops being listed for good.
    pub fn clear_failed_for_asset(&mut self, dataset_id: EntryId, asset_id: &SegmentId) {
        self.0.retain(|registration| {
            registration.dataset_id != dataset_id || registration.failed_asset() != Some(asset_id)
        });
    }

    /// Drops a dataset's failed registrations, keeping the ones that still hold a slot.
    pub fn clear_failed(&mut self, dataset_id: EntryId) {
        self.0.retain(|registration| {
            registration.dataset_id != dataset_id
                || !matches!(registration.state, RegistrationState::Failed(_))
        });
    }

    /// The registrations of one dataset, oldest first.
    pub fn for_dataset(&self, dataset_id: EntryId) -> impl Iterator<Item = &AssetRegistration> {
        self.0
            .iter()
            .filter(move |registration| registration.dataset_id == dataset_id)
    }

    /// The registrations of one dataset that hold an asset slot, oldest first.
    ///
    /// A registration holds a slot from the moment it is started until the dataset's asset list
    /// shows the asset, and its source uri cannot be registered again until then.
    fn pending_for_dataset(&self, dataset_id: EntryId) -> impl Iterator<Item = &AssetRegistration> {
        self.for_dataset(dataset_id)
            .filter(|registration| !matches!(registration.state, RegistrationState::Failed(_)))
    }

    /// Why the server refused this asset, while its registration is still listed.
    ///
    /// The manifest stores the asset but not the reason, so this is the only place it is kept.
    pub fn failure_reason(&self, dataset_id: EntryId, asset_id: &SegmentId) -> Option<&ApiError> {
        self.for_dataset(dataset_id)
            .find_map(|registration| match &registration.state {
                RegistrationState::Failed(err) if err.asset_id.as_ref() == Some(asset_id) => {
                    Some(&err.error)
                }
                RegistrationState::Failed(_)
                | RegistrationState::Pending
                | RegistrationState::Registered => None,
            })
    }

    /// The source uris of a dataset the server is still working on.
    pub fn pending_source_uris(&self, dataset_id: EntryId) -> Vec<String> {
        self.pending_for_dataset(dataset_id)
            .map(|registration| registration.source_uri.clone())
            .collect()
    }

    fn find(&self, dataset_id: EntryId, source_uri: &str) -> Option<usize> {
        self.0.iter().position(|registration| {
            registration.dataset_id == dataset_id && registration.source_uri == source_uri
        })
    }
}

/// An asset a server was asked to drop.
struct AssetUnregistration {
    /// The dataset the asset is registered with.
    dataset_id: EntryId,

    asset_id: SegmentId,

    state: UnregistrationState,
}

enum UnregistrationState {
    /// The server is still working on it.
    Pending,

    /// The server dropped the asset, and the dataset's asset list still shows it.
    Unregistered,
}

/// The assets one server was asked to drop and has yet to answer for.
///
/// The asset keeps its card in the meantime, marked as being unregistered.
#[derive(Default)]
pub struct AssetUnregistrations(Vec<AssetUnregistration>);

impl AssetUnregistrations {
    /// Lists an asset the server has been asked to drop.
    ///
    /// Asking for the same asset again takes over its unregistration.
    pub fn start(&mut self, dataset_id: EntryId, asset_id: SegmentId) {
        let unregistration = AssetUnregistration {
            dataset_id,
            asset_id,
            state: UnregistrationState::Pending,
        };

        match self.find(dataset_id, &unregistration.asset_id) {
            Some(index) => self.0[index] = unregistration,
            None => self.0.push(unregistration),
        }
    }

    /// Records the server's answer.
    ///
    /// An asset the server dropped keeps its mark until the dataset's asset list stops showing it,
    /// while one the server kept loses it right away.
    pub fn finish(&mut self, dataset_id: EntryId, asset_id: &SegmentId, unregistered: bool) {
        let Some(index) = self.find(dataset_id, asset_id) else {
            return;
        };

        if unregistered {
            self.0[index].state = UnregistrationState::Unregistered;
        } else {
            self.0.remove(index);
        }
    }

    /// Whether the asset is on its way out, either because the server is still working on it or
    /// because the asset list has yet to drop it.
    pub fn contains(&self, dataset_id: EntryId, asset_id: &SegmentId) -> bool {
        self.find(dataset_id, asset_id).is_some()
    }

    /// Stops marking the assets the server dropped that the list no longer shows.
    ///
    /// `list_caught_up` says whether a dataset's asset list is done catching up with the server.
    pub fn clear_unregistered(&mut self, list_caught_up: impl Fn(EntryId) -> bool) {
        self.0.retain(|unregistration| {
            matches!(unregistration.state, UnregistrationState::Pending)
                || !list_caught_up(unregistration.dataset_id)
        });
    }

    /// The datasets whose asset list still shows an asset the server unregistered.
    ///
    /// A dataset appears once per such unregistration.
    pub fn datasets_waiting_for_list(&self) -> impl Iterator<Item = EntryId> {
        self.0
            .iter()
            .filter(|unregistration| {
                matches!(unregistration.state, UnregistrationState::Unregistered)
            })
            .map(|unregistration| unregistration.dataset_id)
    }

    fn find(&self, dataset_id: EntryId, asset_id: &SegmentId) -> Option<usize> {
        self.0.iter().position(|unregistration| {
            unregistration.dataset_id == dataset_id && &unregistration.asset_id == asset_id
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refusal from a server that had yet to start registering, so nothing is registered.
    fn server_said_no() -> AssetRegistrationError {
        AssetRegistrationError {
            asset_id: None,
            error: ApiError::internal(&re_uri::Origin::test(), "the server said no"),
        }
    }

    /// A refusal from a server that had already started registering the asset, and keeps it
    /// listed.
    fn server_kept(asset_id: SegmentId) -> AssetRegistrationError {
        AssetRegistrationError {
            asset_id: Some(asset_id),
            error: ApiError::internal(&re_uri::Origin::test(), "the server said no"),
        }
    }

    /// Every dataset's asset list is done catching up with the server.
    fn caught_up(_dataset_id: EntryId) -> bool {
        true
    }

    /// No dataset's asset list has caught up with the server yet.
    fn still_waiting(_dataset_id: EntryId) -> bool {
        false
    }

    fn source_uris(registrations: &AssetRegistrations, dataset_id: EntryId) -> Vec<&str> {
        registrations
            .for_dataset(dataset_id)
            .map(|registration| registration.source_uri.as_str())
            .collect()
    }

    /// A registration is listed for the dataset it was started for, and keeps its slot after the
    /// server took the asset, until the asset list catches up and it is dismissed.
    #[test]
    fn a_registration_is_listed_until_the_asset_list_shows_the_asset() {
        let dataset = EntryId::new();
        let other_dataset = EntryId::new();
        let mut registrations = AssetRegistrations::default();

        registrations.start(dataset, "s3://bucket/first.rrd".to_owned());
        registrations.start(other_dataset, "s3://bucket/other.rrd".to_owned());

        assert_eq!(
            source_uris(&registrations, dataset),
            ["s3://bucket/first.rrd"]
        );
        assert_eq!(
            registrations.pending_source_uris(dataset),
            ["s3://bucket/first.rrd"]
        );

        registrations.finish(dataset, "s3://bucket/first.rrd", Ok(()));

        assert_eq!(
            source_uris(&registrations, dataset),
            ["s3://bucket/first.rrd"]
        );
        assert_eq!(
            registrations.pending_source_uris(dataset),
            ["s3://bucket/first.rrd"]
        );

        // A refresh only clears what the server refused, so the slot is held across one, and so is
        // it while the asset list is still catching up.
        registrations.clear_failed(dataset);
        registrations.clear_registered(still_waiting);
        assert_eq!(registrations.pending_source_uris(dataset).len(), 1);

        registrations.clear_registered(caught_up);

        assert!(source_uris(&registrations, dataset).is_empty());

        // The other dataset's registration is still pending, so its slot outlives the catch-up.
        assert_eq!(source_uris(&registrations, other_dataset).len(), 1);
    }

    /// An asset the server was asked to drop is marked for the dataset it belongs to, and stays
    /// marked after the server dropped it, until the asset list stops showing it.
    #[test]
    fn an_unregistration_is_listed_until_the_asset_list_drops_the_asset() {
        let dataset = EntryId::new();
        let other_dataset = EntryId::new();
        let asset = SegmentId::new("room-mesh".to_owned());
        let mut unregistrations = AssetUnregistrations::default();

        unregistrations.start(dataset, asset.clone());
        unregistrations.start(dataset, asset.clone());

        assert!(unregistrations.contains(dataset, &asset));
        assert!(!unregistrations.contains(other_dataset, &asset));
        assert_eq!(unregistrations.0.len(), 1);

        // The server has yet to answer, so there is nothing for the asset list to catch up with.
        unregistrations.clear_unregistered(caught_up);
        assert!(unregistrations.contains(dataset, &asset));

        unregistrations.finish(dataset, &asset, true);

        assert!(unregistrations.contains(dataset, &asset));

        // The list still shows the asset.
        unregistrations.clear_unregistered(still_waiting);
        assert!(unregistrations.contains(dataset, &asset));

        unregistrations.clear_unregistered(caught_up);

        assert!(!unregistrations.contains(dataset, &asset));
    }

    /// An asset the server kept loses its mark as soon as the server says so, since the asset list
    /// has nothing to catch up with.
    #[test]
    fn an_unregistration_the_server_refused_is_dropped_right_away() {
        let dataset = EntryId::new();
        let asset = SegmentId::new("room-mesh".to_owned());
        let mut unregistrations = AssetUnregistrations::default();

        unregistrations.start(dataset, asset.clone());
        unregistrations.finish(dataset, &asset, false);

        assert!(!unregistrations.contains(dataset, &asset));
    }

    /// A dataset waits for its asset list once the server has answered, until the list has caught
    /// up. A pending registration or unregistration has nothing for the list to catch up with, and
    /// neither has a failed registration.
    #[test]
    fn a_dataset_waits_for_its_asset_list_once_the_server_has_answered() {
        let dataset = EntryId::new();
        let asset = SegmentId::new("room-mesh".to_owned());
        let mut registrations = AssetRegistrations::default();
        let mut unregistrations = AssetUnregistrations::default();

        registrations.start(dataset, "s3://bucket/mesh.rrd".to_owned());
        unregistrations.start(dataset, asset.clone());

        assert_eq!(registrations.datasets_waiting_for_list().count(), 0);
        assert_eq!(unregistrations.datasets_waiting_for_list().count(), 0);

        registrations.finish(dataset, "s3://bucket/mesh.rrd", Ok(()));
        unregistrations.finish(dataset, &asset, true);

        assert_eq!(
            registrations
                .datasets_waiting_for_list()
                .collect::<Vec<_>>(),
            [dataset]
        );
        assert_eq!(
            unregistrations
                .datasets_waiting_for_list()
                .collect::<Vec<_>>(),
            [dataset]
        );

        registrations.clear_registered(caught_up);
        unregistrations.clear_unregistered(caught_up);

        assert_eq!(registrations.datasets_waiting_for_list().count(), 0);
        assert_eq!(unregistrations.datasets_waiting_for_list().count(), 0);

        registrations.start(dataset, "s3://bucket/failed.rrd".to_owned());
        registrations.finish(dataset, "s3://bucket/failed.rrd", Err(server_said_no()));

        assert_eq!(registrations.datasets_waiting_for_list().count(), 0);
    }

    /// A failed registration is listed until it is dismissed or the dataset is refreshed, while a
    /// pending one survives both.
    #[test]
    fn a_failed_registration_is_listed_until_it_is_cleared() {
        let dataset = EntryId::new();
        let mut registrations = AssetRegistrations::default();

        registrations.start(dataset, "s3://bucket/failed.rrd".to_owned());
        registrations.finish(dataset, "s3://bucket/failed.rrd", Err(server_said_no()));
        registrations.start(dataset, "s3://bucket/pending.rrd".to_owned());

        assert_eq!(registrations.for_dataset(dataset).count(), 2);
        assert_eq!(
            registrations.pending_source_uris(dataset),
            ["s3://bucket/pending.rrd"]
        );

        registrations.clear_failed(dataset);

        assert_eq!(
            source_uris(&registrations, dataset),
            ["s3://bucket/pending.rrd"]
        );

        registrations.dismiss(dataset, "s3://bucket/pending.rrd");

        assert!(source_uris(&registrations, dataset).is_empty());
    }

    /// The server writes an asset's row before it reads it, so an asset stays registered even
    /// when the server refuses the registration. The reason it gave is then found by that asset's
    /// id, which is what lets the asset show the failure instead of the registration. A server
    /// that refuses a source before it starts registering registers nothing to find.
    #[test]
    fn a_refused_registration_reports_the_asset_that_stays_registered() {
        let dataset = EntryId::new();
        let kept = SegmentId::new("room-mesh".to_owned());
        let mut registrations = AssetRegistrations::default();

        registrations.start(dataset, "s3://bucket/mesh.rrd".to_owned());
        registrations.finish(
            dataset,
            "s3://bucket/mesh.rrd",
            Err(server_kept(kept.clone())),
        );

        assert_eq!(
            registrations
                .for_dataset(dataset)
                .next()
                .and_then(AssetRegistration::failed_asset),
            Some(&kept)
        );
        assert!(registrations.failure_reason(dataset, &kept).is_some());

        registrations.start(dataset, "s3://bucket/gone.rrd".to_owned());
        registrations.finish(dataset, "s3://bucket/gone.rrd", Err(server_said_no()));

        assert_eq!(registrations.for_dataset(dataset).count(), 2);
        assert!(
            registrations
                .for_dataset(dataset)
                .filter_map(AssetRegistration::failed_asset)
                .eq(std::iter::once(&kept))
        );
    }

    /// Unregistering the asset of a refused registration stops listing that registration, so it
    /// does not show up again once the asset list no longer has the asset. A registration that
    /// registered nothing is untouched, since nothing was dropped on its behalf.
    #[test]
    fn unregistering_the_asset_stops_listing_its_failed_registration() {
        let dataset = EntryId::new();
        let kept = SegmentId::new("room-mesh".to_owned());
        let mut registrations = AssetRegistrations::default();

        registrations.start(dataset, "s3://bucket/mesh.rrd".to_owned());
        registrations.finish(
            dataset,
            "s3://bucket/mesh.rrd",
            Err(server_kept(kept.clone())),
        );
        registrations.start(dataset, "s3://bucket/gone.rrd".to_owned());
        registrations.finish(dataset, "s3://bucket/gone.rrd", Err(server_said_no()));

        registrations.clear_failed_for_asset(dataset, &kept);

        assert_eq!(
            source_uris(&registrations, dataset),
            ["s3://bucket/gone.rrd"]
        );
        assert!(registrations.failure_reason(dataset, &kept).is_none());
    }

    /// Registering the same source uri again takes over its registration instead of listing it
    /// twice, which is what a retry after a failure does.
    #[test]
    fn registering_a_source_uri_again_takes_over_its_registration() {
        let dataset = EntryId::new();
        let mut registrations = AssetRegistrations::default();

        registrations.start(dataset, "s3://bucket/asset.rrd".to_owned());
        registrations.finish(dataset, "s3://bucket/asset.rrd", Err(server_said_no()));
        registrations.start(dataset, "s3://bucket/asset.rrd".to_owned());

        assert_eq!(
            source_uris(&registrations, dataset),
            ["s3://bucket/asset.rrd"]
        );
        assert_eq!(
            registrations.pending_source_uris(dataset),
            ["s3://bucket/asset.rrd"]
        );
    }
}
