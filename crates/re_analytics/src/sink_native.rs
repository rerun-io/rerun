use std::{collections::HashMap, time::Duration};

use once_cell::sync::OnceCell;
use reqwest::{blocking::Client as HttpClient, Url};
use time::OffsetDateTime;

use re_log::{debug, error};

use crate::{Event, Property};

// TODO(cmc): abstract away the concept of a `Sink` behind an actual trait when comes the time to
// support more than just PostHog.

#[cfg(debug_assertions)]
const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_XD1QbqTGdPJbzdVCbvbA9zGOG38wJFTl8RAwqMwBvTY";
#[cfg(not(debug_assertions))]
const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_sgKidIE4WYYFSJHd8LEYY1UZqASpnfQKeMqlJfSXwqg";

// ---

#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

#[derive(Default, Debug, Clone)]
pub struct PostHogSink {
    // NOTE: We need to lazily build the underlying HTTP client, so that we can guarantee that it
    // is initialized from a thread that is free of a Tokio runtime.
    // This is necessary because `reqwest` will crash if we try and initialize a blocking HTTP
    // client from within a thread that has a Tokio runtime instantiated.
    //
    // We also use this opportunity to upgrade our public HTTP endpoint into the final HTTP2/TLS
    // URL by following all 301 redirects.
    client: OnceCell<(Url, HttpClient)>,
}

impl PostHogSink {
    /// Our public entrypoint; this will be resolved into an actual HTTP2/TLS URL when creating
    /// the client.
    const URL: &str = "http://tel.rerun.io";

    pub fn send(
        &self,
        analytics_id: &str,
        session_id: &str,
        events: &[Event],
    ) -> Result<(), SinkError> {
        let (resolved_url, client) = self.init()?;

        let events = events
            .iter()
            .map(|event| PostHogEvent::from_event(analytics_id, session_id, event))
            .collect::<Vec<_>>();
        let batch = PostHogBatch::from_events(&events);

        debug!("{}", serde_json::to_string_pretty(&batch)?);
        let resp = client
            .post(resolved_url.clone())
            .body(serde_json::to_vec(&batch)?)
            .send()?;

        resp.error_for_status().map(|_| ()).map_err(Into::into)
    }

    fn init(&self) -> Result<&(Url, HttpClient), SinkError> {
        self.client.get_or_try_init(|| {
            use reqwest::header;
            let mut headers = header::HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            );

            let client = HttpClient::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(5))
                .pool_idle_timeout(Duration::from_secs(120))
                .default_headers(headers)
                .build()?;

            let resolved_url = client.get(Self::URL).send()?.url().clone();

            Ok((resolved_url, client))
        })
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
