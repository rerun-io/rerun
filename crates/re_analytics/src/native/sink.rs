use std::collections::HashMap;
use std::sync::Arc;

use time::OffsetDateTime;

use super::AbortSignal;
use crate::{Event, Property};

// TODO(cmc): abstract away the concept of a `Sink` behind an actual trait when comes the time to
// support more than just PostHog.

const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_sgKidIE4WYYFSJHd8LEYY1UZqASpnfQKeMqlJfSXwqg";

// ---

#[derive(Debug, Clone)]
struct Url(String);

#[derive(Default, Debug, Clone)]
pub(crate) struct PostHogSink {}

impl PostHogSink {
    /// Our public telemetry endpoint.
    const URL: &str = "https://tel.rerun.io";

    #[allow(clippy::unused_self)]
    pub(crate) fn send(
        &self,
        analytics_id: &Arc<str>,
        session_id: &Arc<str>,
        events: &[Event],
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

// ---

// TODO(cmc): support PostHog's view event

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
enum PostHogEvent<'a> {
    Capture(PostHogCaptureEvent<'a>),
    Identify(PostHogIdentifyEvent<'a>),
}

impl<'a> PostHogEvent<'a> {
    fn from_event(analytics_id: &'a str, session_id: &'a str, event: &'a Event) -> Self {
        let properties = event.props.iter().map(|(name, value)| {
            (
                name.as_ref(),
                match value {
                    &Property::Integer(v) => v.into(),
                    &Property::Float(v) => v.into(),
                    Property::String(v) => v.as_str().into(),
                    &Property::Bool(v) => v.into(),
                },
            )
        });

        match event.kind {
            crate::EventKind::Append => Self::Capture(PostHogCaptureEvent {
                timestamp: event.time_utc,
                event: event.name.as_ref(),
                distinct_id: analytics_id,
                properties: properties
                    .chain([("session_id", session_id.into())])
                    .collect(),
            }),
            crate::EventKind::Update => Self::Identify(PostHogIdentifyEvent {
                timestamp: event.time_utc,
                event: "$identify",
                distinct_id: analytics_id,
                properties: [("session_id", session_id.into())].into(),
                set: properties.collect(),
            }),
        }
    }
}

// See https://posthog.com/docs/api/post-only-endpoints#capture.
#[derive(Debug, serde::Serialize)]
struct PostHogCaptureEvent<'a> {
    #[serde(with = "::time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
}

// See https://posthog.com/docs/api/post-only-endpoints#identify.
#[derive(Debug, serde::Serialize)]
struct PostHogIdentifyEvent<'a> {
    #[serde(with = "::time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
    #[serde(rename = "$set")]
    set: HashMap<&'a str, serde_json::Value>,
}

// See https://posthog.com/docs/api/post-only-endpoints#batch-events.
#[derive(Debug, serde::Serialize)]
struct PostHogBatch<'a> {
    api_key: &'static str,
    batch: &'a [PostHogEvent<'a>],
}

impl<'a> PostHogBatch<'a> {
    fn from_events(events: &'a [PostHogEvent<'a>]) -> Self {
        Self {
            api_key: PUBLIC_POSTHOG_PROJECT_KEY,
            batch: events,
        }
    }
}
