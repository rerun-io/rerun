#![expect(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]

use std::sync::Arc;
use std::time::Duration;

use crate::{AnalyticsEvent, Config, PostHogBatch, PostHogEvent};

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

/// Wasm event pipeline.
///
/// Unlike the native pipeline, this one is not backed by a WAL. All events are immediately sent as they are recorded.
#[derive(Debug)]
pub struct Pipeline {
    analytics_id: Arc<str>,
    session_id: Arc<str>,
}

impl Pipeline {
    // NOTE: different from the native URL, this one is _specifically_ for web.
    const URL: &'static str = "https://tel.rerun.io/api/pog";

    pub(crate) fn new(config: &Config, _tick: Duration) -> Result<Option<Self>, PipelineError> {
        Ok(Some(Self {
            analytics_id: config.analytics_id.as_str().into(),
            session_id: config.session_id.to_string().into(),
        }))
    }

    pub fn record(&self, event: AnalyticsEvent) {
        // send all events immediately, ignore all errors

        let analytics_id = self.analytics_id.clone();
        let session_id = self.session_id.clone();

        let events = [PostHogEvent::from_event(
            &self.analytics_id,
            &self.session_id,
            &event,
        )];
        let batch = PostHogBatch::from_events(&events);
        let json = match serde_json::to_string_pretty(&batch) {
            Ok(json) => json,
            Err(err) => {
                re_log::debug_once!("failed to send event: {err}");
                return;
            }
        };
        re_log::trace!("Sending analytics: {json}");
        ehttp::fetch(
            ehttp::Request::post(Self::URL, json.into_bytes()),
            move |result| match result {
                Ok(response) => {
                    if !response.ok {
                        re_log::debug_once!(
                            "Failed to send analytics down the sink: HTTP request failed: {} {} {}",
                            response.status,
                            response.status_text,
                            response.text().unwrap_or("")
                        );
                        return;
                    }

                    re_log::trace!(
                        ?response,
                        %analytics_id,
                        %session_id,
                        "events successfully flushed"
                    );
                }
                Err(err) => {
                    re_log::debug_once!("Failed to send analytics down the sink: {err}");
                }
            },
        );
    }
}
