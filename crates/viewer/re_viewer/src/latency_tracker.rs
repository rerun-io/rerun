//! Utility to track rtt latency for redap origins.

use std::sync::Arc;

use ahash::HashMap;
use re_mutex::Mutex;
use re_redap_client::ConnectionRegistryHandle;

pub const MIN_QUERY_INTERVAL: web_time::Duration = web_time::Duration::from_secs(1);

pub enum LatencyResult {
    /// The most recent ping to the server failed.
    NoConnection,

    /// We haven't gotten back the answer for the first ping yet.
    ToBeAssigned,

    /// The most recently measured latency.
    MostRecent(web_time::Duration),
}

#[derive(Default)]
struct InnerState {
    accessed: bool,
    update_in_progress: bool,
    has_error: bool,
    last_latency: Option<web_time::Duration>,
    last_update_time: Option<web_time::Instant>,
}

#[derive(Default)]
struct LatencyTracker {
    pub inner: Mutex<InnerState>,
}

impl re_byte_size::SizeBytes for LatencyTracker {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
}

impl LatencyTracker {
    fn should_update(&self) -> bool {
        let lock = self.inner.lock();
        !lock.update_in_progress
            && lock.accessed
            && lock
                .last_update_time
                .is_none_or(|t| t.elapsed() >= MIN_QUERY_INTERVAL)
    }

    fn error(&self) {
        let mut lock = self.inner.lock();
        lock.has_error = true;
        lock.update_in_progress = false;
    }

    fn update(&self) {
        let mut lock = self.inner.lock();

        let Some(last_update) = lock.last_update_time else {
            return;
        };

        lock.last_latency = Some(last_update.elapsed());

        lock.accessed = false;
        lock.update_in_progress = false;
        lock.has_error = false;
    }

    fn latency(&self) -> LatencyResult {
        let mut lock = self.inner.lock();
        lock.accessed = true;
        if lock.has_error {
            return LatencyResult::NoConnection;
        }

        if let Some(latency) = lock.last_latency {
            LatencyResult::MostRecent(latency)
        } else {
            LatencyResult::ToBeAssigned
        }
    }
}

#[derive(Default)]
pub struct ServerLatencyTrackers {
    servers: HashMap<re_uri::Origin, Arc<LatencyTracker>>,
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    crate::external::tokio::spawn(future);
}

impl ServerLatencyTrackers {
    /// Ping all origins if they're in use and enough time has passed since the
    /// last ping.
    pub fn update(&self, connection_registry_handle: &ConnectionRegistryHandle) {
        #[expect(clippy::iter_over_hash_type)] // Order doesn't matter here.
        for (origin, tracker) in &self.servers {
            if !tracker.should_update() {
                continue;
            }

            tracker.inner.lock().update_in_progress = true;

            let tracker = tracker.clone();

            let origin = origin.clone();
            let handle = connection_registry_handle.clone();
            spawn_future(async move {
                tracker.inner.lock().last_update_time = Some(web_time::Instant::now());

                let Ok(mut client) = handle.client(origin).await else {
                    tracker.error();
                    return;
                };

                if client.ping().await.is_err() {
                    tracker.error();
                    return;
                }

                tracker.update();
            });
        }
    }

    /// Get the most recent latency for an origin.
    pub fn origin_latency(&mut self, origin: &re_uri::Origin) -> LatencyResult {
        self.servers.entry(origin.clone()).or_default().latency()
    }
}
