// TODO: turn the following into proper crate-level doc.
//
// Config manager
// ==============
//
// $XDG_CONFIG/rerun/analytics.json
//
// If it does not exist, we interpret it as "first run".
//
// On first run, we print a disclaimer, and never send any data.
//
// CLI
// ===
//
// Just a user-friendly way to work with the config file.
//
// `rerun analytics <clear|opt-out>`
//
// Native path
// ===========
//
// Everything happens in two different threads: file writer and file POSTer.
//
// File writer:
// - Always append to a file on disk
//   - just keep it perma open, leave it to the FS cache
// - Based on space and time: send the fd to the file POSTer thread
//
// File POSTer:
// - Receives a fd and tries to POST it to posthog
//   - on success, deletes it
//   - on failure, tries again later
//
// Questions:
// - Is there any part of this that we can reasonably test?
// - Anyway we can simulate posthog being down?
//
// Web path
// ========
//
// Everything has to happen in the same thread, obviously :/
// It's gotta be frame-based.
//
// - Just synchronously dump events into local store
// - Every N frame (or something along that line), send it to posthog
//   - on success, deletes it
//   - on failure, tries again later
//
// Questions:
// - Can we just write synchronously to local store? is that fast enough?
// - Can we even do a POST through web-sys? Can we detect failure???

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use re_log::trace;
use time::OffsetDateTime;

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    Append,
    Update,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    // NOTE: serialized in a human-readable format as we want end users to be able to inspect the
    // data we send out.
    // TODO: is UTC fine? do we care about user's tz?
    #[serde(with = "::time::serde::rfc3339")]
    pub time_utc: OffsetDateTime,
    pub kind: EventKind,
    pub name: Cow<'static, str>,
    pub props: HashMap<String, Property>,
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

    pub fn with_prop(mut self, name: String, value: impl Into<Property>) -> Self {
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

// TODO: needs a real copy here!
const DISCLAIMER: &str = "
    Welcome to Rerun!

    Summary:
    - This open source library collects anonymous usage statistics.
    - We cannot see and do not store information contained inside Rerun apps,
      such as text logs, images, point clouds, etc.
    - Telemetry data is stored in servers in Europe.
    - If you'd like to opt out, run the following: `rerun analytics disable`.

    You can check out all of our telemetry events in `re_analytics/src/all_events.rs`.

    As this is this your first session, we will _not_ send out any telemetry data yet,
    giving you an opportunity to opt-out first.

    You can audit the actual data being sent out by looking at our data directory at the
    end of your session:
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
    pipeline: Pipeline,

    default_props: HashMap<String, Property>,
    event_id: AtomicU64,
}

impl Analytics {
    pub fn new(
        tick: Duration,
        default_props: HashMap<String, Property>,
    ) -> Result<Self, AnalyticsError> {
        let config = Config::load()?;
        trace!(?config, ?tick, "loaded analytics config");

        if config.is_first_run() {
            eprintln!("{DISCLAIMER}");
            eprintln!("    {:?}\n", config.data_dir());

            config.save()?;
            trace!(?config, ?tick, "saved analytics config");
        }

        let sink = PostHogSink::new()?;
        let pipeline = Pipeline::new(&config, tick, sink)?;

        if config.is_first_run() {
            pipeline.record(Event::update_metadata());
        }

        Ok(Self {
            config,
            default_props,
            pipeline,
            event_id: AtomicU64::new(1),
        })
    }

    pub fn record(&self, mut event: Event) {
        if self.config.analytics_enabled {
            // Insert default props
            if event.kind == EventKind::Append {
                event.props.extend(self.default_props.clone());
            }

            // Insert event ID
            event.props.insert(
                "event_id".into(),
                (self.event_id.fetch_add(1, Ordering::Relaxed) as i64).into(),
            );

            self.pipeline.record(event);
        }
    }
}

// ---

mod config;
use self::config::{Config, ConfigError};

pub mod events;

// TODO(cmc): web pipeline
mod pipeline;
use self::pipeline::{Pipeline, PipelineError};

// TODO(cmc): web sink
mod sink;
use self::sink::{PostHogSink, SinkError};

pub mod cli;
