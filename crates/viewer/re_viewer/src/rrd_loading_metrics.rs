use std::time::Instant;

/// Metrics for tracking RRD loading performance from start to playback completion.
///
/// This tracks the complete end-to-end flow when loading RRD files with the --profile flag
#[derive(Default, Debug, Clone)]
pub struct RrdLoadingMetrics {
    /// When we started loading the RRD file
    pub loading_start: Option<Instant>,

    /// When the data source was successfully opened
    pub data_source_opened: Option<Instant>,

    /// When we received the first message
    pub first_message_received: Option<Instant>,

    /// When we received the last message (data source disconnected)
    pub last_message_received: Option<Instant>,

    /// When all messages have been fully ingested into `EntityDb`
    pub all_messages_ingested: Option<Instant>,

    /// When automatic playback completed
    pub playback_completed: Option<Instant>,

    /// Total messages received from data source
    pub total_messages: usize,

    /// Total messages ingested into `EntityDb` (may lag behind received in parallel ingestion)
    pub total_ingested: usize,

    /// Whether we're currently tracking a load operation
    pub is_tracking: bool,

    /// Path or URL being loaded
    pub source_path: Option<String>,
}

impl RrdLoadingMetrics {
    /// Start tracking metrics for a new RRD load operation.
    pub fn start_tracking(&mut self, source_path: String) {
        *self = Self {
            loading_start: Some(Instant::now()),
            source_path: Some(source_path),
            is_tracking: true,
            ..Default::default()
        };
        re_log::debug!("Started tracking RRD loading metrics");
    }

    /// Mark that the data source was successfully opened.
    pub fn mark_data_source_opened(&mut self) {
        if self.is_tracking {
            self.data_source_opened = Some(Instant::now());
        }
    }

    /// Mark that the first message was received.
    pub fn mark_first_message(&mut self) {
        if self.is_tracking && self.first_message_received.is_none() {
            self.first_message_received = Some(Instant::now());
        }
    }

    /// Mark that a message was received from the data source.
    pub fn mark_message_received(&mut self) {
        if self.is_tracking {
            self.total_messages += 1;
            self.last_message_received = Some(Instant::now());
        }
    }

    /// Mark that a message was ingested into `EntityDb`.
    pub fn mark_message_ingested(&mut self) {
        if self.is_tracking {
            self.total_ingested += 1;
        }
    }

    /// Mark that all messages have been ingested into `EntityDb`.
    pub fn mark_all_ingested(&mut self) {
        if self.is_tracking && self.all_messages_ingested.is_none() {
            self.all_messages_ingested = Some(Instant::now());
            re_log::debug!(
                "All {} messages ingested into EntityDb",
                self.total_ingested
            );
        }
    }

    /// Mark that automatic playback has completed.
    pub fn mark_playback_complete(&mut self) {
        if self.is_tracking && self.playback_completed.is_none() {
            self.playback_completed = Some(Instant::now());
            self.is_tracking = false;
            self.emit_metrics();
        }
    }

    /// Check if metrics collection is complete.
    #[expect(dead_code)]
    pub fn is_complete(&self) -> bool {
        self.playback_completed.is_some()
    }

    /// Calculate end-to-end loading time from start to playback completion.
    pub fn total_loading_time(&self) -> Option<std::time::Duration> {
        match (self.loading_start, self.playback_completed) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    /// Calculate time from loading start to first message received.
    pub fn time_to_first_message(&self) -> Option<std::time::Duration> {
        match (self.loading_start, self.first_message_received) {
            (Some(start), Some(first)) => Some(first.duration_since(start)),
            _ => None,
        }
    }

    /// Calculate time from data source opening to first message.
    pub fn data_source_to_first_message(&self) -> Option<std::time::Duration> {
        match (self.data_source_opened, self.first_message_received) {
            (Some(opened), Some(first)) => Some(first.duration_since(opened)),
            _ => None,
        }
    }

    /// Calculate ingestion lag time (last message received to all ingested).
    ///
    /// This is particularly useful for measuring parallel ingestion,
    /// where messages are processed on a background thread.
    pub fn ingestion_lag_time(&self) -> Option<std::time::Duration> {
        match (self.last_message_received, self.all_messages_ingested) {
            (Some(last_rx), Some(all_ingested)) => Some(all_ingested.duration_since(last_rx)),
            _ => None,
        }
    }

    /// Calculate playback time (all ingested to playback complete).
    pub fn playback_time(&self) -> Option<std::time::Duration> {
        match (self.all_messages_ingested, self.playback_completed) {
            (Some(ingested), Some(complete)) => Some(complete.duration_since(ingested)),
            _ => None,
        }
    }

    /// Calculate message throughput in messages per second.
    pub fn throughput_msgs_per_sec(&self) -> Option<f64> {
        self.total_loading_time()
            .map(|duration| self.total_messages as f64 / duration.as_secs_f64())
    }

    /// Emit metrics to console log.
    fn emit_metrics(&self) {
        if let Some(total_time) = self.total_loading_time() {
            re_log::info!("");
            re_log::info!("╔════════════════════════════════════════════════════════════════╗");
            re_log::info!("║          RRD Loading Metrics - Playback Complete              ║");
            re_log::info!("╚════════════════════════════════════════════════════════════════╝");
            re_log::info!("");
            re_log::info!("  Total Loading Time:  {:.3}s", total_time.as_secs_f64());

            if let Some(ttfm) = self.time_to_first_message() {
                re_log::info!(
                    "  Time to First Msg:   {:.3}s ({:.1}%)",
                    ttfm.as_secs_f64(),
                    (ttfm.as_secs_f64() / total_time.as_secs_f64()) * 100.0
                );
            }

            if let Some(lag) = self.ingestion_lag_time() {
                re_log::info!(
                    "  Ingestion Lag:       {:.3}s ({:.1}%)",
                    lag.as_secs_f64(),
                    (lag.as_secs_f64() / total_time.as_secs_f64()) * 100.0
                );
            }

            if let Some(playback) = self.playback_time() {
                re_log::info!(
                    "  Playback Time:       {:.3}s ({:.1}%)",
                    playback.as_secs_f64(),
                    (playback.as_secs_f64() / total_time.as_secs_f64()) * 100.0
                );
            }

            re_log::info!("");
            re_log::info!("  Total Messages:      {}", self.total_messages);

            if let Some(throughput) = self.throughput_msgs_per_sec() {
                re_log::info!("  Throughput:          {:.0} msgs/sec", throughput);
            }

            if let Some(path) = &self.source_path {
                re_log::info!("  Source:              {}", path);
            }

            re_log::info!("");
            re_log::info!("════════════════════════════════════════════════════════════════");
            re_log::info!("");
        }
    }

    /// Get a summary string for UI display.
    #[expect(dead_code)]
    pub fn summary_string(&self) -> String {
        if let Some(total_time) = self.total_loading_time() {
            format!(
                "Loaded {} messages in {:.2}s ({:.0} msgs/sec)",
                self.total_messages,
                total_time.as_secs_f64(),
                self.throughput_msgs_per_sec().unwrap_or(0.0)
            )
        } else if self.is_tracking {
            format!(
                "Loading... {} received, {} ingested",
                self.total_messages, self.total_ingested
            )
        } else {
            "No active loading operation".to_owned()
        }
    }
}
