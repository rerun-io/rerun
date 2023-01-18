use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, Write},
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::{
    channel::{self, RecvError},
    select,
};
use reqwest::blocking::Client as HttpClient;
use time::OffsetDateTime;

use re_log::{error, trace};

use crate::{Config, Event, Property};

// ---

// TODO: no idea how we're supposed to deal with some entity actively trashing our analytics?
// only way I can think of is to go through our own server first and have asymetric encryption...
const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_XD1QbqTGdPJbzdVCbvbA9zGOG38wJFTl8RAwqMwBvTY";

// ---

#[derive(thiserror::Error, Debug)]
pub enum SinkError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

// TODO(cmc): abstract away when comes the day where we want to re-use the pipeline for another
// provider.
// TODO: actually maybe abstract away right now at this point...

// TODO: view event

#[derive(Debug)]
pub struct PostHogSink {
    client: HttpClient,
}

impl PostHogSink {
    pub fn new() -> Result<Self, SinkError> {
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

        Ok(Self { client })
    }

    // TODO: blocking!
    pub fn send(
        &self,
        analytics_id: &str,
        session_id: &str,
        events: &[Event],
    ) -> Result<(), SinkError> {
        const URL: &str = "https://eu.posthog.com/capture";

        let events = events
            .iter()
            .map(|event| PostHogEvent::from_event(analytics_id, session_id, event))
            .collect::<Vec<_>>();
        let batch = PostHogBatch::from_events(&events);

        eprintln!("{}", serde_json::to_string_pretty(&batch)?);
        let resp = self
            .client
            .post(URL)
            .body(serde_json::to_vec(&batch)?)
            .send()?;

        resp.error_for_status().map(|_| ()).map_err(Into::into)
    }
}

// ---

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
                name.as_str(),
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
                    .chain([
                        // TODO: surely there has to be some nicer way of dealing with sessions...
                        ("session_id", session_id.into()),
                    ])
                    // TODO: application_id (hashed)
                    // TODO: recording_id (hashed)
                    // (unless these belong only to viewer-opened?)
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

// See https://posthog.com/docs/api/post-only-endpoints#single-event.
#[derive(Debug, serde::Serialize)]
struct PostHogCaptureEvent<'a> {
    #[serde(with = "::time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
}

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
