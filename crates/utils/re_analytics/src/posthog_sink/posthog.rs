use std::collections::HashMap;

use jiff::Timestamp;

use crate::{AnalyticsEvent, Property};

/// The "public" API key can be obtained at <https://eu.posthog.com/project/settings#project-api-key>.
/// Make sure you are logged in to the right organization and have the correct project open.
/// Unfortunately that stuff is client-side routed, and there's no way to link directly to the right place.
pub const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_sgKidIE4WYYFSJHd8LEYY1UZqASpnfQKeMqlJfSXwqg";

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum PostHogEvent<'a> {
    Capture(PostHogCaptureEvent<'a>),
    Identify(PostHogIdentifyEvent<'a>),
    Alias(PostHogSetPersonPropertiesEvent<'a>),
}

impl<'a> PostHogEvent<'a> {
    pub fn from_event(
        analytics_id: &'a str,
        session_id: &'a str,
        event: &'a AnalyticsEvent,
    ) -> Self {
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
            crate::EventKind::Identify => Self::Identify(PostHogIdentifyEvent {
                timestamp: event.time_utc,
                event: "$identify",
                distinct_id: analytics_id,
                properties: [("session_id", session_id.into())].into(),
                set: properties.collect(),
            }),
            crate::EventKind::SetPersonProperties => Self::Alias(PostHogSetPersonPropertiesEvent {
                timestamp: event.time_utc,
                event: "$set",
                distinct_id: analytics_id,
                properties: [("$set", properties.collect())].into(),
            }),
        }
    }
}

// See https://posthog.com/docs/api/post-only-endpoints#capture.
#[derive(Debug, serde::Serialize)]
pub struct PostHogCaptureEvent<'a> {
    timestamp: Timestamp,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
}

// See https://posthog.com/docs/api/post-only-endpoints#identify.
#[derive(Debug, serde::Serialize)]
pub struct PostHogIdentifyEvent<'a> {
    timestamp: Timestamp,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
    #[serde(rename = "$set")]
    set: HashMap<&'a str, serde_json::Value>,
}

// See https://posthog.com/docs/product-analytics/person-properties.
#[derive(Debug, serde::Serialize)]
pub struct PostHogSetPersonPropertiesEvent<'a> {
    timestamp: Timestamp,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
}

// See https://posthog.com/docs/api/post-only-endpoints#batch-events.
#[derive(Debug, serde::Serialize)]
pub struct PostHogBatch<'a> {
    api_key: &'static str,
    batch: &'a [PostHogEvent<'a>],
}

impl<'a> PostHogBatch<'a> {
    pub fn from_events(events: &'a [PostHogEvent<'a>]) -> Self {
        Self {
            api_key: PUBLIC_POSTHOG_PROJECT_KEY,
            batch: events,
        }
    }
}
