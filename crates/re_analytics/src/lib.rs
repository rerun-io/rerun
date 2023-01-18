use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use re_log::trace;
use time::OffsetDateTime;

// ---

// TODO: `analytics_id` and `session_id` have to be stored here! no way around it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Event {
    // NOTE: serialized in a human-readable format as we want end users to be able to inspect the
    // data we send out.
    // TODO: is UTC fine? do we care about user's tz?
    #[serde(with = "::time::serde::rfc3339")]
    pub time_utc: OffsetDateTime,
    // TODO: the static string forces people to list it as part of src/events.
    pub name: Cow<'static, str>,
    pub props: HashMap<String, Property>,
}

impl Event {
    pub fn new(name: Cow<'static, str>) -> Self {
        Self {
            time_utc: OffsetDateTime::now_utc(),
            name,
            props: Default::default(),
        }
    }

    pub fn with_prop(mut self, name: String, value: impl Into<Property>) -> Self {
        self.props.insert(name, value.into());
        self
    }
}

// TODO: guess that's more than enough for now...?
#[derive(Debug, Clone, derive_more::From, serde::Serialize, serde::Deserialize)]
pub enum Property {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

// ---

// TODO: web too

const DISCLAIMER: &str = "
    Welcome to Rerun!

    Summary:
    - This open source library collects anonymous usage statistics.
    - We cannot see and do not store information contained inside Rerun apps,
      such as text logs, images, point clouds, etc.
    - Telemetry data is stored in servers in Europe.
    - If you'd like to opt out, run the following: `rerun analytics disable`.

    As this is this your first session, we will _not_ send out any telemetry data yet,
    giving you an opportunity to opt-out first.

    You can audit the data being sent out by looking at our data directory at the end of your session:
";

#[derive(thiserror::Error, Debug)]
pub enum AnalyticsError {
    #[error(transparent)]
    ConfigError(#[from] ConfigError),

    #[error(transparent)]
    PipelineError(#[from] PipelineError),
}

pub struct Analytics {
    config: Config,
    pipeline: EventPipeline,
}

impl Analytics {
    // TODO: fill with logs
    pub fn new(tick: Duration) -> Result<Self, AnalyticsError> {
        let config = Config::load()?;
        trace!(?config, ?tick, "loaded analytics config");

        if config.is_first_run() {
            // TODO: that's when we display analytics disclaimer in terminal!
            //
            // if this file doens't exist, this is the first run
            //     - we never send data on the first run
            //     - we print to terminal everything analytics related on first run
            //     - and we make it clear we havent sent anything
            //     - we can point out where the actual data lives
            eprintln!("{DISCLAIMER}");
            eprintln!("    {:?}\n", config.data_dir());

            config.save()?;
        }

        let pipeline = EventPipeline::new(&config, tick)?;

        Ok(Self { config, pipeline })
    }

    pub fn record(&self, event: Event) {
        if self.config.analytics_enabled {
            self.pipeline.record(event);
        }
    }
}

// ---

// TODO: flag everything cfg(native) vs. cfg(web)
// TODO: all of this should prob be private

mod config;
pub use self::config::{Config, ConfigError};

pub mod events;

mod pipeline;
pub use self::pipeline::{EventPipeline, PipelineError};

pub mod cli;

// TODO: events and pipeline/buffering
// TODO: backends and posthog
// TODO: cli (extends the existing one!)

// Config manager
// ==============
//
// $XDG_CONFIG/rerun/analytics.json
//
// If it does not exist, we interpret it as "first run".
//
// On first run, we print a disclaimer, and never send any data.

// CLI
// ===
//
// Just a user-friendly way to work with the config file.
//
// `rerun analytics <clear|opt-out>`

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
