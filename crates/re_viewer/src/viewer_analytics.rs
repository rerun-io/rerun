//! All telemetry analytics collected by the Rerun Viewer are defined in this file for easy auditing.
//!
//! Analytics can be disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.

#[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
use re_analytics::{Analytics, Event, Property};

pub struct ViewerAnalytics {
    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled
    // while at the same time opting-out of analytics at run-time.
    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    analytics: Option<Analytics>,
}

impl ViewerAnalytics {
    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    pub fn new() -> Self {
        let analytics = match Analytics::new(std::time::Duration::from_secs(2)) {
            Ok(analytics) => Some(analytics),
            Err(err) => {
                re_log::error!(%err, "failed to initialize analytics SDK");
                None
            }
        };

        Self { analytics }
    }

    #[cfg(not(all(not(target_arch = "wasm32"), feature = "analytics")))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    fn record(&self, event: Event) {
        if let Some(analytics) = &self.analytics {
            analytics.record(event);
        }
    }

    /// Register a property that will be included in all append-events.
    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    fn register(&mut self, name: &'static str, property: impl Into<Property>) {
        if let Some(analytics) = &mut self.analytics {
            analytics.register_append_property(name, property);
        }
    }
}

// ----------------------------------------------------------------------------

/// Here follows all the analytics collected by the Rerun Viewer.
#[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
impl ViewerAnalytics {
    pub fn on_viewer_started(&self) {
        // TODO(cmc): start_method
        //
        // We want to know (I think..?):
        // - Loading rrd (either URL or file, whatever)
        // - Standalone boot, waiting for network data
        // - Standalone boot, feeding from another server (??)
        // - SDK boot, show()
        // - SDK book, spawn()

        #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
        if let Some(analytics) = &self.analytics {
            let rerun_version = env!("CARGO_PKG_VERSION");
            let rust_version = env!("CARGO_PKG_RUST_VERSION");
            let target = re_analytics::TARGET_TRIPLET;
            let git_hash = re_analytics::GIT_HASH;

            let mut event = Event::update("update_metadata".into())
                .with_prop("rerun_version".into(), rerun_version.to_owned())
                .with_prop("rust_version".into(), rust_version.to_owned())
                .with_prop("target".into(), target.to_owned())
                .with_prop("git_hash".into(), git_hash.to_owned());

            // Append opt-in metadata.
            // In practice this is the email of Rerun employees
            // who register their emails with `rerun analytics email`.
            // This is how we filter out employees from actual users!
            for (name, value) in analytics.config().opt_in_metadata.clone() {
                event = event.with_prop(name.into(), value);
            }

            analytics.record(event);
        }

        self.record(Event::append("viewer_started".into()));
    }

    pub fn on_new_recording(&mut self, msg: &re_log_types::BeginRecordingMsg) {
        // We hash the application_id and recording_id unless this is an official example.
        // That's because we want to be able to track which are the popular examples,
        // but we don't want to collect actual application ids.
        self.register("application_id", {
            let prop = Property::from(msg.info.application_id.0.clone());
            if msg.info.is_official_example {
                prop
            } else {
                prop.hashed()
            }
        });
        self.register("recording_id", {
            let prop = Property::from(msg.info.recording_id.to_string());
            if msg.info.is_official_example {
                prop
            } else {
                prop.hashed()
            }
        });
        self.register("recording_source", msg.info.recording_source.to_string());
        self.register("is_official_example", msg.info.is_official_example);
        self.record(Event::append("data_source_opened".into()));
    }
}

// ----------------------------------------------------------------------------

// When analytics are disabled:
#[cfg(not(all(not(target_arch = "wasm32"), feature = "analytics")))]
impl ViewerAnalytics {
    pub fn on_viewer_started(&self) {}
    pub fn on_new_recording(&self, _msg: &re_log_types::BeginRecordingMsg) {}
}
