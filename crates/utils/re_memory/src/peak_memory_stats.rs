//! Monitor memory use periodically, and capture the stacktraces at the high water mark.

use std::thread::JoinHandle;

use crossbeam::channel::{Receiver, Sender};

use crate::TrackingStatistics;
use crate::accounting_allocator::{self, is_tracking_callstacks};

#[derive(Debug)]
struct Shutdown;

/// Collects [`TrackingStatistics`] at the point of peak memory pressure.
///
/// Internally this runs a thread that periodically checks the current memory usage.
/// At any high water mark, new [`TrackingStatistics`] is collected and stored.
///
/// This is primarily meant for developer investigation.
pub struct PeakMemoryStats {
    shutdown_tx: Sender<Shutdown>,
    handle: JoinHandle<Option<TrackingStatistics>>,
}

impl PeakMemoryStats {
    /// Start a background thread, tracking peak memory use.
    pub fn start() -> Self {
        if !is_tracking_callstacks() {
            re_log::warn_once!(
                "Callstack tracking is disabled - peak memory use will not be collected"
            );
        }

        let (shutdown_tx, shutdown_rx) = crossbeam::channel::bounded(1);
        let handle = std::thread::Builder::new()
            .name("PeakMemoryStates".to_owned())
            .spawn(move || collect_peak_memory_use(&shutdown_rx))
            .expect("Failed to spawn PeakMemoryStates thread");
        Self {
            shutdown_tx,
            handle,
        }
    }

    pub fn finish(self) -> Option<TrackingStatistics> {
        let Self {
            shutdown_tx,
            handle,
        } = self;

        re_quota_channel::send_crossbeam(&shutdown_tx, Shutdown).ok();

        match handle.join() {
            Err(err) => {
                re_log::warn_once!("Failed to collect PeakMemoryStates: {err:?}"); // NOLINT: err does not implement Display
                None
            }
            Ok(result) => result,
        }
    }
}

fn current_memory_use() -> usize {
    if let Some(stats) = crate::accounting_allocator::global_allocs() {
        stats.size
    } else {
        re_log::error_once!("accounting_allocator is OFF. Can't collect peak memory use");
        0
    }
}

fn collect_peak_memory_use(shutdown_rx: &Receiver<Shutdown>) -> Option<TrackingStatistics> {
    // How often we check RAM use.
    let interval = std::time::Duration::from_secs(5);

    let watermark_margin = 100 * 1024 * 1024;

    let mut highest_memory_use = current_memory_use();
    let mut highest_so_far = accounting_allocator::tracking_stats();

    loop {
        // Wait for either a shutdown signal or a 10-second timeout
        crossbeam::select! {
            recv(shutdown_rx) -> _ => {
                // Shutdown signal received
                return highest_so_far;
            }
            default(interval) => {
                let current_memory_use = current_memory_use();

                if highest_memory_use + watermark_margin < current_memory_use {
                    highest_memory_use = current_memory_use;
                    highest_so_far = accounting_allocator::tracking_stats();
                }
            }
        }
    }
}
