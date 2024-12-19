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
pub struct AnalyticsEvent {
    // NOTE: serialized in a human-readable format as we want end users to be able to inspect the
    // data we send out.
    #[serde(with = "::time::serde::rfc3339")]
    time_utc: OffsetDateTime,
    kind: EventKind,
    name: Cow<'static, str>,
    props: HashMap<Cow<'static, str>, Property>,
}

impl AnalyticsEvent {
    #[inline]
    pub fn new(name: impl Into<Cow<'static, str>>, kind: EventKind) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
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
    event_id: AtomicU64,
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

impl Analytics {
    /// Initialize an analytics pipeline which flushes events every `tick`.
    pub fn new(tick: Duration) -> Result<Self, AnalyticsError> {
        let config = load_config()?;
        let pipeline = Pipeline::new(&config, tick)?;
        re_log::trace!("initialized analytics pipeline");

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
                    (self.event_id.fetch_add(1, Ordering::Relaxed) as i64).into(),
                );
            }

            pipeline.record(event);
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
        } = self;

        event.insert("features", features);
        event.insert("git_hash", git_hash);
        event.insert("rerun_version", version.to_string());
        event.insert("rust_version", rustc_version);
        event.insert("llvm_version", llvm_version);
        event.insert("target", target_triple);
        event.insert("build_date", datetime);
        event.insert("debug", cfg!(debug_assertions)); // debug-build?
        event.insert("rerun_workspace", is_in_rerun_workspace);
    }
}
