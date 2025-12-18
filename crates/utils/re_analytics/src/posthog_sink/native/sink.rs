use std::sync::Arc;

use super::AbortSignal;
use crate::{AnalyticsEvent, PostHogBatch, PostHogEvent};

#[derive(Default, Debug, Clone)]
pub(crate) struct PostHogSink {}

impl PostHogSink {
    /// Our public telemetry endpoint.
    const URL: &'static str = "https://tel.rerun.io";

    #[expect(clippy::unused_self)]
    pub(crate) fn send(
        &self,
        analytics_id: &Arc<str>,
        session_id: &Arc<str>,
        events: &[AnalyticsEvent],
        abort_signal: &AbortSignal,
    ) {
        let num_events = events.len();
        let on_done = {
            let analytics_id = analytics_id.clone();
            let session_id = session_id.clone();
            let abort_signal = abort_signal.clone();
            move |result: Result<ehttp::Response, String>| match result {
                Ok(response) => {
                    if !response.ok {
                        let err = format!(
                            "HTTP request failed: {} {} {}",
                            response.status,
                            response.status_text,
                            response.text().unwrap_or("")
                        );
                        re_log::debug!("Failed to send analytics down the sink: {err}");
                        return abort_signal.abort();
                    }

                    re_log::trace!(
                        ?response,
                        %analytics_id,
                        %session_id,
                        num_events = num_events,
                        "events successfully flushed"
                    );
                }
                Err(err) => {
                    re_log::debug!("Failed to send analytics down the sink: {err}");
                    abort_signal.abort();
                }
            }
        };

        let events = events
            .iter()
            .map(|event| PostHogEvent::from_event(analytics_id, session_id, event))
            .collect::<Vec<_>>();
        let batch = PostHogBatch::from_events(&events);

        let json = match serde_json::to_string_pretty(&batch) {
            Ok(json) => json,
            Err(err) => return on_done(Err(err.to_string())),
        };
        re_log::trace!("Sending analytics: {json}");
        ehttp::fetch(ehttp::Request::post(Self::URL, json.into_bytes()), on_done);
    }
}
