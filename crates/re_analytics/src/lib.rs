//! Rerun's analytics SDK.
//!
//! We never collect any personal identifiable information, and you can always opt-out with `rerun analytics disable`.
//!
//! All the data we collect can be found in
//! <https://github.com/rerun-io/rerun/blob/latest/crates/re_viewer/src/viewer_analytics.rs>.

// We never use any log levels other than `trace` and `debug` because analytics is not important
// enough to require the attention of our users.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{Config, ConfigError};
#[cfg(not(target_arch = "wasm32"))]
use native::{Pipeline, PipelineError};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{Config, ConfigError};
#[cfg(target_arch = "wasm32")]
use web::{Pipeline, PipelineError};

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

// ----------------------------------------------------------------------------

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Error as IoError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use time::OffsetDateTime;

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    /// Append a new event to the time series associated with this analytics ID.
    ///
    /// Used e.g. to send an event every time the app start.
    Append,

    /// Update the permanent state associated with this analytics ID.
    ///
    /// Used e.g. to associate an OS with a particular analytics ID upon its creation.
    Update,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    // NOTE: serialized in a human-readable format as we want end users to be able to inspect the
    // data we send out.
    #[serde(with = "::time::serde::rfc3339")]
    pub time_utc: OffsetDateTime,
    pub kind: EventKind,
    pub name: Cow<'static, str>,
    pub props: HashMap<Cow<'static, str>, Property>,
}

impl Event {
    pub fn append(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
            kind: EventKind::Append,
            name: name.into(),
            props: Default::default(),
        }
    }

    pub fn update(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
            kind: EventKind::Update,
            name: name.into(),
            props: Default::default(),
        }
    }

    /// NOTE: due to an earlier snafu, we filter out all properties called
    /// `git_branch` and `location` on the server-end, so don't use those property names!
    /// See <https://github.com/rerun-io/rerun/pull/1563> for details.
    pub fn with_prop(
        mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Property>,
    ) -> Self {
        let name = name.into();
        debug_assert!(
            name != "git_branch" && name != "location",
            "We filter out the property name {name:?} on the server-end. Pick a different name."
        );
        self.props.insert(name, value.into());
        self
    }

    /// Adds Rerun version, git hash, build date and similar as properties to the event.
    pub fn with_build_info(self, build_info: &re_build_info::BuildInfo) -> Event {
        let re_build_info::BuildInfo {
            crate_name: _,
            version,
            rustc_version,
            llvm_version,
            git_hash: _,
            git_branch: _,
            is_in_rerun_workspace,
            target_triple,
            datetime,
        } = build_info;

        // We intentionally don't include the branch name, because it can contain sensitive user-stuff.

        self.with_prop("rerun_version", version.to_string())
            .with_prop("rust_version", (*rustc_version).to_owned())
            .with_prop("llvm_version", (*llvm_version).to_owned())
            .with_prop("target", *target_triple)
            .with_prop("git_hash", build_info.git_hash_or_tag())
            .with_prop("build_date", *datetime)
            .with_prop("debug", cfg!(debug_assertions)) // debug-build?
            .with_prop("rerun_workspace", *is_in_rerun_workspace)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Property {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl Property {
    /// Returns a new string property that is a hex representation of the hashed sum of the current
    /// property.
    pub fn hashed(&self) -> Property {
        /// Just a random fixed salt to render pre-built rainbow tables useless.
        const SALT: &str = "d6d6bed3-028a-49ac-94dc-8c89cfb19379";

        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::default();
        hasher.update(SALT);
        match self {
            Property::Bool(data) => hasher.update([*data as u8]),
            Property::Integer(data) => hasher.update(data.to_le_bytes()),
            Property::Float(data) => hasher.update(data.to_le_bytes()),
            Property::String(data) => hasher.update(data),
        }
        Self::String(format!("{:x}", hasher.finalize()))
    }
}

impl From<bool> for Property {
    #[inline]
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Property {
    #[inline]
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for Property {
    #[inline]
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<String> for Property {
    #[inline]
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for Property {
    #[inline]
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

// ---

const DISCLAIMER: &str = "
    Welcome to Rerun!

    This open source library collects anonymous usage data to
    help the Rerun team improve the library.

    Summary:
    - We only collect high level events about the features used within the Rerun Viewer.
    - The actual data you log to Rerun, such as point clouds, images, or text logs,
      will never be collected.
    - We don't log IP addresses.
    - We don't log your user name, file paths, or any personal identifiable data.
    - Usage data we do collect will be sent to and stored on servers within the EU.

    For more details and instructions on how to opt out, run the command:

      rerun analytics details

    As this is this your first session, _no_ usage data has been sent yet,
    giving you an opportunity to opt-out first if you wish.

    Happy Rerunning!
";

#[derive(thiserror::Error, Debug)]
pub enum AnalyticsError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Pipeline(#[from] PipelineError),

    #[error(transparent)]
    Io(#[from] IoError),
}

pub struct Analytics {
    config: Config,

    /// `None` if analytics are disabled.
    pipeline: Option<Pipeline>,

    default_append_props: HashMap<Cow<'static, str>, Property>,
    event_id: AtomicU64,
}

impl Analytics {
    pub fn new(tick: Duration) -> Result<Self, AnalyticsError> {
        let config = Config::load()?;

        #[cfg(target_arch = "wasm32")]
        let config = if let Some(config) = config {
            config
        } else {
            // the config doesnt exist in local storage yet, save it
            let config = Config::default();
            config.save()?;
            config
        };

        #[cfg(not(target_arch = "wasm32"))]
        let config = match config {
            Some(config) => config,
            None => Config::new()?,
        };

        re_log::trace!(?config, ?tick, "loaded analytics config");

        if config.is_first_run() {
            eprintln!("{DISCLAIMER}");

            config.save()?;
            re_log::trace!(?config, ?tick, "saved analytics config");
        }

        let pipeline = Pipeline::new(&config, tick)?;

        Ok(Self {
            config,
            default_append_props: Default::default(),
            pipeline,
            event_id: AtomicU64::new(1), // we skip 0 just to be explicit (zeroes can often be implicit)
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Register a property that will be included in all [`EventKind::Append`].
    pub fn register_append_property(&mut self, name: &'static str, prop: impl Into<Property>) {
        self.default_append_props.insert(name.into(), prop.into());
    }

    /// Deregister a property.
    pub fn deregister_append_property(&mut self, name: &'static str) {
        self.default_append_props.remove(name);
    }

    /// Record an event.
    ///
    /// It will be extended with an `event_id` and, if this is an [`EventKind::Append`],
    /// any properties registered with [`Self::register_append_property`].
    pub fn record(&self, mut event: Event) {
        if let Some(pipeline) = self.pipeline.as_ref() {
            if event.kind == EventKind::Append {
                // Insert default props
                event.props.extend(self.default_append_props.clone());

                // Insert event ID
                event.props.insert(
                    "event_id".into(),
                    (self.event_id.fetch_add(1, Ordering::Relaxed) as i64).into(),
                );
            }

            pipeline.record(event);
        }
    }
}

const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_sgKidIE4WYYFSJHd8LEYY1UZqASpnfQKeMqlJfSXwqg";

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
