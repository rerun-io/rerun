//! The viewer's connections to servers, grouped so that `App` holds them in one field.

use re_log_channel::LogSource;
use re_redap_client::ConnectionRegistryHandle;
use re_viewer_context::viewer_url::ViewerUrl;

use crate::latency_tracker::ServerLatencyTrackers;
#[cfg(not(target_arch = "wasm32"))]
use crate::registrations_in_flight::RegisteredFiles;
use crate::registrations_in_flight::RegistrationsInFlight;

/// The viewer's connections to servers, and what it tracks about them.
pub struct ServerConnections {
    /// Every server the viewer knows about, including the internal catalog.
    pub registry: ConnectionRegistryHandle,

    /// Measures the latency to each server in use.
    pub latency_trackers: ServerLatencyTrackers,

    /// The registrations with any of the catalogs that are in progress.
    pub registrations_in_flight: RegistrationsInFlight,

    /// The dataset segments each local file was registered as.
    #[cfg(not(target_arch = "wasm32"))]
    pub registered_files: RegisteredFiles,
}

impl ServerConnections {
    /// No latency measured and no registrations in progress yet.
    pub fn new(registry: ConnectionRegistryHandle) -> Self {
        Self {
            registry,
            latency_trackers: ServerLatencyTrackers::default(),
            registrations_in_flight: RegistrationsInFlight::default(),
            #[cfg(not(target_arch = "wasm32"))]
            registered_files: RegisteredFiles::default(),
        }
    }

    /// The sources that `url` loads from, as the viewer records them on its recordings.
    ///
    /// A file registered with the viewer's catalog loads from the dataset segments it was
    /// registered as.
    #[cfg_attr(target_arch = "wasm32", expect(clippy::unused_self))]
    pub fn sources_of_url(&self, url: &ViewerUrl) -> Vec<LogSource> {
        let sources = url.data_sources();
        cfg_select! {
            target_arch = "wasm32" => { sources }
            _ => {
                let segments: Vec<LogSource> = sources
                    .iter()
                    .filter_map(|source| match source {
                        LogSource::File { path } => Some(path),
                        _ => None,
                    })
                    .flat_map(|path| self.registered_files.segments(path))
                    .map(|segment| LogSource::RedapGrpcStream {
                        uri: segment,
                        open_behavior: re_log_channel::RecordingOpenBehavior::Background,
                    })
                    .collect();
                std::iter::chain(sources, segments).collect()
            }
        }
    }
}
