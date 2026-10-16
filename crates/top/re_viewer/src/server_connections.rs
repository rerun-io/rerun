//! The viewer's connections to servers, grouped so that `App` holds them in one field.

use re_redap_client::ConnectionRegistryHandle;

use crate::latency_tracker::ServerLatencyTrackers;
use crate::registrations_in_flight::RegistrationsInFlight;

/// The viewer's connections to servers, and what it tracks about them.
pub struct ServerConnections {
    /// Every server the viewer knows about, including the internal catalog.
    pub registry: ConnectionRegistryHandle,

    /// Measures the latency to each server in use.
    pub latency_trackers: ServerLatencyTrackers,

    /// The registrations with any of the catalogs that are in progress.
    pub registrations_in_flight: RegistrationsInFlight,
}

impl ServerConnections {
    /// No latency measured and no registrations in progress yet.
    pub fn new(registry: ConnectionRegistryHandle) -> Self {
        Self {
            registry,
            latency_trackers: ServerLatencyTrackers::default(),
            registrations_in_flight: RegistrationsInFlight::default(),
        }
    }
}
