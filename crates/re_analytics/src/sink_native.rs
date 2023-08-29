use std::collections::HashMap;

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
    #[error("HTTP transport: {0}")]
    HttpTransport(Box<ureq::Transport>),

    #[error("HTTP status {status_code} {status_text}: {body}")]
    HttpStatus {
        status_code: u16,
        status_text: String,
        body: String,
    },
}

impl From<ureq::Error> for SinkError {
    fn from(err: ureq::Error) -> Self {
        match err {
            ureq::Error::Status(status_code, response) => Self::HttpStatus {
                status_code,
                status_text: response.status_text().to_owned(),
                body: response.into_string().unwrap_or_default(),
            },

            ureq::Error::Transport(transport) => Self::HttpTransport(Box::new(transport)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostHogSink {
    agent: ureq::Agent,
    // Lazily resolve the url so that we don't do blocking requests in `PostHogSink::default`
    resolved_url: once_cell::sync::OnceCell<String>,
}

impl Default for PostHogSink {
    fn default() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(5))
                .build(),
            resolved_url: Default::default(),
        }
    }
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
        let resolved_url = self.init()?;

        let events = events
            .iter()
            .map(|event| PostHogEvent::from_event(analytics_id, session_id, event))
            .collect::<Vec<_>>();
        let batch = PostHogBatch::from_events(&events);

        re_log::trace!(
            "Sending analytics: {}",
            serde_json::to_string_pretty(&batch)?
        );
        self.agent.post(resolved_url).send_json(&batch)?;
        Ok(())
    }

    fn init(&self) -> Result<&String, SinkError> {
        self.resolved_url.get_or_try_init(|| {
            // Make a dummy-request to resolve our final URL.
            let resolved_url = match self.agent.get(Self::URL).call() {
                Ok(response) => response.get_url().to_owned(),
                Err(ureq::Error::Status(status, response)) => {
                    // We actually expect to get here, because we make a bad request (GET to and end-point that expects a POST).
                    // We only do this requests to get redirected to the final URL.
                    let resolved_url = response.get_url().to_owned();
                    re_log::trace!("status: {status} {}", response.status_text().to_owned());
                    resolved_url
                }
                Err(ureq::Error::Transport(transport)) => {
                    return Err(SinkError::HttpTransport(Box::new(transport)))
                }
            };

            // 2023-08-25 the resolved URL was https://tel.rerun.io/

            Ok(resolved_url)
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
