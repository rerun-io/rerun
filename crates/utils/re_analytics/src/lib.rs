//! Rerun's analytics SDK.
//!
//! We never collect any personal identifiable information.
//! You can always opt-out with `rerun analytics disable`.
//!
//! No analytics will be collected the first time you start the Rerun viewer,
//! giving you an opportunity to opt-out first if you wish.
//!
//! All the data we collect can be found in [`event`].

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

mod posthog;
use posthog::{PostHogBatch, PostHogEvent};

pub mod event;

// ----------------------------------------------------------------------------

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Error as IoError;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use jiff::Timestamp;

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    /// Append a new event to the time series associated with this analytics ID.
    ///
    /// Used e.g. to send an event every time the app start.
    Append,

    /// Collect data about the environment upon startup.
    ///
    /// Used to associate the host machine's OS, Rust version, etc. with the analytics ID.
    Identify,

    /// Set properties of an authenticated user.
    ///
    /// Used to set the user's email address after they log in so that we can link
    /// anonymous analytics IDs to the authenticated users.
    SetPersonProperties,
}

// ----------------------------------------------------------------------------

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error("Analytics connection closed before flushing completed")]
    Closed,

    #[error("Flush timed out - not all analytics messages were sent.")]
    Timeout,
}

// ----------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalyticsEvent {
    time_utc: Timestamp,
    kind: EventKind,
    name: Cow<'static, str>,
    props: HashMap<Cow<'static, str>, Property>,
}

impl AnalyticsEvent {
    #[inline]
    pub fn new(name: impl Into<Cow<'static, str>>, kind: EventKind) -> Self {
        Self {
            time_utc: Timestamp::now(),
            kind,
            name: name.into(),
            props: Default::default(),
        }
    }

    /// Insert a property into the event, overwriting any existing property with the same name.
    #[inline]
    pub fn insert(&mut self, name: impl Into<Cow<'static, str>>, value: impl Into<Property>) {
        self.props.insert(name.into(), value.into());
    }

    /// Insert a property into the event, but only if its `value` is `Some`,
    /// in which case any existing property with the same name will be overwritten.
    ///
    /// This has no effect if `value` is `None`.
    #[inline]
    pub fn insert_opt(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: Option<impl Into<Property>>,
    ) {
        if let Some(value) = value {
            self.props.insert(name.into(), value.into());
        }
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
    pub fn hashed(&self) -> Self {
        /// Just a random fixed salt to render pre-built rainbow tables useless.
        const SALT: &str = "d6d6bed3-028a-49ac-94dc-8c89cfb19379";

        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::default();
        hasher.update(SALT);
        match self {
            Self::Bool(data) => hasher.update([*data as u8]),
            Self::Integer(data) => hasher.update(data.to_le_bytes()),
            Self::Float(data) => hasher.update(data.to_le_bytes()),
            Self::String(data) => hasher.update(data),
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

impl From<f32> for Property {
    #[inline]
    fn from(value: f32) -> Self {
        Self::Float(value as _)
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

#[cfg(not(target_arch = "wasm32"))]
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
    event_id: AtomicI64,
}

#[cfg(not(target_arch = "wasm32"))] // NOTE: can't block on web
impl Drop for Analytics {
    fn drop(&mut self) {
        if let Some(pipeline) = self.pipeline.as_ref()
            && let Err(err) = pipeline.flush_blocking(Duration::MAX)
        {
            re_log::debug!("Failed to flush analytics events during shutdown: {err}");
        }
    }
}

fn load_config() -> Result<Config, ConfigError> {
    let config = match Config::load() {
        Ok(config) => config,

        Err(err) => {
            // NOTE: This will cause the first run disclaimer to show up again on native,
            //       and analytics will be disabled for the rest of the session.
            if !cfg!(target_arch = "wasm32") {
                re_log::warn!("failed to load analytics config file: {err}");
            }
            None
        }
    };

    if let Some(config) = config {
        re_log::trace!(?config, "loaded analytics config");

        Ok(config)
    } else {
        re_log::trace!(?config, "initialized analytics config");

        // NOTE: If this fails, we give up, because we can't produce
        //       a config on native any other way.
        let config = Config::new()?;

        #[cfg(not(target_arch = "wasm32"))]
        if config.is_first_run() {
            eprintln!("{DISCLAIMER}");

            config.save()?;
            re_log::trace!(?config, "saved analytics config");
        }

        #[cfg(target_arch = "wasm32")]
        {
            // always save the config on web, without printing a disclaimer.
            config.save()?;
            re_log::trace!(?config, "saved analytics config");
        }

        Ok(config)
    }
}

static GLOBAL_ANALYTICS: OnceLock<Option<Analytics>> = OnceLock::new();

impl Analytics {
    /// Get the global analytics instance, initializing it if it's not already initialized.
    ///
    /// Return `None` if analytics is disabled or some error occurred.
    pub fn global_or_init() -> Option<&'static Self> {
        GLOBAL_ANALYTICS
            .get_or_init(|| match Self::new(Duration::from_secs(2)) {
                Ok(analytics) => Some(analytics),
                Err(err) => {
                    re_log::error!("Failed to initialize analytics: {err}");
                    None
                }
            })
            .as_ref()
    }

    /// Get the global analytics instance, but only if it has already been initialized with [`Self::global_or_init`].
    ///
    /// Return `None` if analytics is disabled or some error occurred during initialization.
    ///
    /// Usually it is better to use [`Self::global_or_init`] instead.
    pub fn global_get() -> Option<&'static Self> {
        GLOBAL_ANALYTICS.get()?.as_ref()
    }

    /// Initialize an analytics pipeline which flushes events every `tick`.
    ///
    /// Usually it is better to use [`Self::global_or_init`] instead of calling this directly,
    /// but there are cases where you might want to create a separate instance,
    /// e.g. for testing purposes, or when you want to use a different tick duration.
    fn new(tick: Duration) -> Result<Self, AnalyticsError> {
        let config = load_config()?;
        let pipeline = Pipeline::new(&config, tick)?;
        re_log::trace!("initialized analytics pipeline");

        Ok(Self {
            config,
            default_append_props: Default::default(),
            pipeline,
            event_id: AtomicI64::new(1), // we skip 0 just to be explicit (zeroes can often be implicit)
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Record a single event.
    ///
    /// The event is constructed using the implementations of [`Event`] and [`Properties`].
    /// The event's properties will be extended with an `event_id`.
    pub fn record<E: Event>(&self, event: E) {
        if self.pipeline.is_none() {
            return;
        }

        let mut e = AnalyticsEvent::new(E::NAME, E::KIND);
        event.serialize(&mut e);
        self.record_raw(e);
    }

    #[cfg(not(target_arch = "wasm32"))] // NOTE: can't block on web
    pub fn flush_blocking(&self, timeout: Duration) -> Result<(), FlushError> {
        if let Some(pipeline) = self.pipeline.as_ref() {
            pipeline.flush_blocking(timeout)
        } else {
            Ok(())
        }
    }

    /// Record an event.
    ///
    /// It will be extended with an `event_id`.
    fn record_raw(&self, mut event: AnalyticsEvent) {
        if let Some(pipeline) = self.pipeline.as_ref() {
            if event.kind == EventKind::Append {
                // Insert default props
                event.props.extend(self.default_append_props.clone());

                // Insert event ID
                event.props.insert(
                    "event_id".into(),
                    self.event_id.fetch_add(1, Ordering::Relaxed).into(),
                );
            }

            pipeline.record(event);
        }
    }
}

pub fn record<E: Event>(cb: impl FnOnce() -> E) {
    if let Some(analytics) = Analytics::global_or_init() {
        analytics.record(cb());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn record_and_flush_blocking<E: Event>(cb: impl FnOnce() -> E) {
    if let Some(analytics) = Analytics::global_or_init() {
        analytics.record(cb());
        if let Err(err) = analytics.flush_blocking(std::time::Duration::MAX)
            && cfg!(debug_assertions)
        {
            eprintln!("Failed to flush analytics: {err}");
        }
    }
}

/// An analytics event.
///
/// This trait requires an implementation of [`Properties`].
pub trait Event: Properties {
    /// The name of the event.
    ///
    /// We prefer `snake_case` when naming events.
    const NAME: &'static str;

    /// What kind of event this is.
    ///
    /// Most events do not update state, so the default here is [`EventKind::Append`].
    const KIND: EventKind = EventKind::Append;
}

/// Trait representing the properties of an analytics event.
///
/// This is separate from [`Event`] to facilitate code re-use.
///
/// For example, [`re_build_info::BuildInfo`] has an implementation of this trait,
/// so that any event which wants to include build info in its properties
/// may include that struct in its own definition, and then call `build_info.serialize`
/// in its own `serialize` implementation.
pub trait Properties: Sized {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let _ = event;
    }
}

impl Properties for re_build_info::BuildInfo {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let git_hash = self.git_hash_or_tag();
        let Self {
            crate_name: _,
            features,
            version,
            rustc_version,
            llvm_version,
            git_hash: _,
            git_branch: _,
            is_in_rerun_workspace,
            target_triple,
            datetime,
            is_debug_build,
        } = self;

        event.insert("features", features.to_string());
        event.insert("git_hash", git_hash);
        event.insert("rerun_version", version.to_string());
        event.insert("rust_version", rustc_version.to_string());
        event.insert("llvm_version", llvm_version.to_string());
        event.insert("target", target_triple.to_string());
        event.insert("build_date", datetime.to_string());
        event.insert("debug", is_debug_build);
        event.insert("rerun_workspace", is_in_rerun_workspace);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn test_analytics_event_serialization() {
        // Create an event using the new jiff implementation
        let mut event = AnalyticsEvent::new("test_event", EventKind::Append);
        event.insert("test_property", "test_value");

        // Serialize to JSON
        let serialized = serde_json::to_string(&event).expect("Failed to serialize event");
        let parsed: Value = serde_json::from_str(&serialized).expect("Failed to parse JSON");

        // Verify the timestamp format is correct (RFC3339)
        let time_str = parsed["time_utc"]
            .as_str()
            .expect("time_utc should be a string");

        // The format should be like: "2025-04-03T01:20:10.557958200Z"
        // RFC3339 regex pattern
        let re = regex_lite::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$")
            .expect("Failed to compile regex");

        assert!(
            re.is_match(time_str),
            "Timestamp '{time_str}' does not match expected RFC3339 format",
        );

        // Verify other fields
        assert_eq!(parsed["kind"], "Append");
        assert_eq!(parsed["name"], "test_event");

        // Check the property structure - it's an object with "String" field
        let property = &parsed["props"]["test_property"];
        assert!(property.is_object(), "Property should be an object");
        assert_eq!(property["String"], "test_value");
    }

    #[test]
    fn test_timestamp_now_behavior() {
        // Create an event
        let event = AnalyticsEvent::new("test_event", EventKind::Append);

        // Verify the timestamp is close to now
        // This ensures jiff::Timestamp::now() behavior matches time::OffsetDateTime::now_utc()
        let now = jiff::Timestamp::now();
        let event_time = event.time_utc;

        // The timestamps should be within a few seconds of each other
        let diff = (now.as_nanosecond() - event_time.as_nanosecond()).abs();
        let five_seconds_ns = 5_000_000_000;

        assert!(
            diff < five_seconds_ns,
            "Timestamp difference is too large: {diff} nanoseconds"
        );
    }
}
