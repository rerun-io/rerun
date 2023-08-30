use std::collections::HashMap;
use std::sync::Arc;

use time::OffsetDateTime;

use crate::{Event, Property};

// TODO(cmc): abstract away the concept of a `Sink` behind an actual trait when comes the time to
// support more than just PostHog.

const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_sgKidIE4WYYFSJHd8LEYY1UZqASpnfQKeMqlJfSXwqg";

// ---

#[derive(Debug, Clone)]
struct Url(String);

#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error("File seek error: {0}")]
    FileSeek(std::io::Error),

    #[error("JSON: {0}")]
    Serde(#[from] serde_json::Error),

    /// Usually because there is no internet.
    #[error("HTTP: {0}")]
    Http(String),

    #[error("HTTP status {status_code} {status_text}: {body}")]
    HttpStatus {
        status_code: u16,
        status_text: String,
        body: String,
    },
}

#[derive(Default, Debug, Clone)]
pub struct PostHogSink {}

impl PostHogSink {
    /// Our public telemetry endpoint.
    const URL: &str = "https://tel.rerun.io";

    #[allow(clippy::unused_self)]
    pub fn send(&self, analytics_id: &Arc<str>, session_id: &Arc<str>, events: &[Event]) {
        let num_events = events.len();
        let on_done = {
            let analytics_id = analytics_id.clone();
            let session_id = session_id.clone();
            move |result: Result<ehttp::Response, String>| match result {
                Ok(response) => {
                    re_log::trace!(
                        ?response,
                        %analytics_id,
                        %session_id,
                        num_events = num_events,
                        "events successfully flushed"
                    );
                }
                Err(error) => {
                    re_log::debug_once!(
                        "Failed to send analytics down the sink, will try again later.\n{error}"
                    );
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
            Err(error) => return on_done(Err(error.to_string())),
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
