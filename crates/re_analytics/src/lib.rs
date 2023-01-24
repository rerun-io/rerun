//! Rerun's analytics SDK.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use re_log::trace;
use time::OffsetDateTime;

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    /// Append a new event to the time series associated with this analytics ID.
    ///
    /// Used e.g. to send an event everytime the app start.
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
    pub fn append(name: Cow<'static, str>) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
            kind: EventKind::Append,
            name,
            props: Default::default(),
        }
    }

    pub fn update(name: Cow<'static, str>) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
            kind: EventKind::Update,
            name,
            props: Default::default(),
        }
    }

    pub fn with_prop(mut self, name: Cow<'static, str>, value: impl Into<Property>) -> Self {
        self.props.insert(name, value.into());
        self
    }
}

#[derive(Debug, Clone, derive_more::From, serde::Serialize, serde::Deserialize)]
pub enum Property {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

// ---

const DISCLAIMER: &str = "
    Welcome to Rerun!

    Summary:
    - This open source library collects anonymous usage data
      to help the Rerun team improve the library.
    - The data you pass to Rerun apps, such as point clouds, images, or text logs,
      will never be collected.
    - Collected usage data is sent to and stored in servers within the EU.
    - For more details and instructions on how to opt out, run: `rerun analytics details`.

    As this is this your first session, _no_ usage data has been sent yet,
    giving you an opportunity to opt-out first.
";

#[derive(thiserror::Error, Debug)]
pub enum AnalyticsError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Pipeline(#[from] PipelineError),

    #[error(transparent)]
    Sink(#[from] SinkError),
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
        trace!(?config, ?tick, "loaded analytics config");

        if config.is_first_run() {
            eprintln!("{DISCLAIMER}");

            config.save()?;
            trace!(?config, ?tick, "saved analytics config");
        }

        let sink = PostHogSink::default();
        let pipeline = Pipeline::new(&config, tick, sink)?;

        if let Some(pipeline) = pipeline.as_ref() {
            pipeline.record(Event::update_metadata());
        }

        Ok(Self {
            config,
            default_append_props: Default::default(),
            pipeline,
            event_id: AtomicU64::new(1),
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn default_append_props_mut(&mut self) -> &mut HashMap<Cow<'static, str>, Property> {
        &mut self.default_append_props
    }

    pub fn record(&self, mut event: Event) {
        if let Some(pipeline) = self.pipeline.as_ref() {
            // Insert default props
            if event.kind == EventKind::Append {
                event.props.extend(self.default_append_props.clone());
            }

            // Insert event ID
            event.props.insert(
                "event_id".into(),
                (self.event_id.fetch_add(1, Ordering::Relaxed) as i64).into(),
            );

            pipeline.record(event);
        }
    }
}

// ---

#[cfg(not(target_arch = "wasm32"))]
mod config_native;
#[cfg(not(target_arch = "wasm32"))]
use self::config_native::{Config, ConfigError};

#[cfg(target_arch = "wasm32")]
mod config_web;
#[cfg(target_arch = "wasm32")]
use self::config_web::{Config, ConfigError};

#[cfg(not(target_arch = "wasm32"))]
mod pipeline_native;
#[cfg(not(target_arch = "wasm32"))]
use self::pipeline_native::{Pipeline, PipelineError};

// TODO(cmc): web pipeline
#[cfg(target_arch = "wasm32")]
mod pipeline_web;
#[cfg(target_arch = "wasm32")]
use self::pipeline_web::{Pipeline, PipelineError};

#[cfg(not(target_arch = "wasm32"))]
mod sink_native;
#[cfg(not(target_arch = "wasm32"))]
use self::sink_native::{PostHogSink, SinkError};

// TODO(cmc): web sink
#[cfg(target_arch = "wasm32")]
mod sink_web;
#[cfg(target_arch = "wasm32")]
use self::sink_web::{PostHogSink, SinkError};

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;

pub mod events;
