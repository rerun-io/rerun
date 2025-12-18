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

// ----------------------------------------------------------------------------

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error("Analytics connection closed before flushing completed")]
    Closed,

    #[error("Flush timed out - not all analytics messages were sent.")]
    Timeout,
}
