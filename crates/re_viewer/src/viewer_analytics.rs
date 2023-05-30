//! All telemetry analytics collected by the Depthai Viewer are defined in this file for easy auditing.
//!
//! There are two exceptions:
//! * `crates/rerun/src/crash_handler.rs` sends anonymized callstacks on crashes
//! * `crates/re_web_viewer_server/src/lib.rs` sends an anonymous event when a `.wasm` web-viewer is served.
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.
//!
//! DO NOT MOVE THIS FILE without updating all the docs pointing to it!

#[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
use re_analytics::{Analytics, Event, Property};

#[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
use re_log_types::RecordingSource;

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

    /// Deregister a property.
    #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
    fn deregister(&mut self, name: &'static str) {
        if let Some(analytics) = &mut self.analytics {
            analytics.deregister_append_property(name);
        }
    }
}

// ----------------------------------------------------------------------------

/// Here follows all the analytics collected by the Depthai Viewer.
#[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
impl ViewerAnalytics {
    /// When the viewer is first started
    pub fn on_viewer_started(
        &mut self,
        build_info: &re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
    ) {
        use crate::AppEnvironment;
        let app_env_str = match app_env {
            AppEnvironment::PythonSdk(..) => "python_sdk",
            AppEnvironment::RustSdk { .. } => "rust_sdk",
            AppEnvironment::RerunCli { .. } => "rerun_cli",
            AppEnvironment::Web => "web_viewer",
        };
        self.register("app_env", app_env_str);

        #[cfg(all(not(target_arch = "wasm32"), feature = "analytics"))]
        if let Some(analytics) = &self.analytics {
            let mut event = Event::update("update_metadata").with_build_info(build_info);

            // If we happen to know the Python or Rust version used on the _host machine_, i.e. the
            // machine running the viewer, then add it to the permanent user profile.
            //
            // The Python/Rust versions appearing in user profiles always apply to the host
            // environment, _not_ the environment in which the data logging is taking place!
            match &app_env {
                AppEnvironment::RustSdk {
                    rustc_version,
                    llvm_version,
                }
                | AppEnvironment::RerunCli {
                    rustc_version,
                    llvm_version,
                } => {
                    event = event.with_prop("rust_version", rustc_version.clone());
                    event = event.with_prop("llvm_version", llvm_version.clone());
                }
                _ => {}
            }
            if let AppEnvironment::PythonSdk(version, ..) = app_env {
                event = event.with_prop("python_version", version.to_string());
            }

            // Append opt-in metadata.
            // In practice this is the email of Rerun employees
            // who register their emails with `rerun analytics email`.
            // This is how we filter out employees from actual users!
            for (name, value) in analytics.config().opt_in_metadata.clone() {
                event = event.with_prop(name, value);
            }

            analytics.record(event);
        }

        self.record(Event::append("viewer_started"));
    }

    /// When we have loaded the start of a new recording.
    pub fn on_open_recording(&mut self, log_db: &re_data_store::LogDb) {
        if let Some(rec_info) = log_db.recording_info() {
            // We hash the application_id and recording_id unless this is an official example.
            // That's because we want to be able to track which are the popular examples,
            // but we don't want to collect actual application ids.
            self.register("application_id", {
                let prop = Property::from(rec_info.application_id.0.clone());
                if rec_info.is_official_example {
                    prop
                } else {
                    prop.hashed()
                }
            });
            self.register("recording_id", {
                let prop = Property::from(rec_info.recording_id.to_string());
                if rec_info.is_official_example {
                    prop
                } else {
                    prop.hashed()
                }
            });

            let recording_source = match &rec_info.recording_source {
                RecordingSource::Unknown => "unknown".to_owned(),
                RecordingSource::PythonSdk(_version, ..) => "python_sdk".to_owned(),
                RecordingSource::RustSdk { .. } => "rust_sdk".to_owned(),
                RecordingSource::Other(other) => other.clone(),
            };

            // If we happen to know the Python or Rust version used on the _recording machine_,
            // then append it to all future events.
            //
            // The Python/Rust versions appearing in events always apply to the recording
            // environment, _not_ the environment in which the viewer is running!
            if let RecordingSource::RustSdk {
                rustc_version: rust_version,
                llvm_version,
            } = &rec_info.recording_source
            {
                self.register("rust_version", rust_version.to_string());
                self.register("llvm_version", llvm_version.to_string());
                self.deregister("python_version"); // can't be both!
            }
            if let RecordingSource::PythonSdk(version, ..) = &rec_info.recording_source {
                self.register("python_version", version.to_string());
                self.deregister("rust_version"); // can't be both!
                self.deregister("llvm_version"); // can't be both!
            }

            self.register("recording_source", recording_source);
            self.register("is_official_example", rec_info.is_official_example);
        }

        if let Some(data_source) = &log_db.data_source {
            let data_source = match data_source {
                re_smart_channel::Source::File { .. } => "file", // .rrd
                re_smart_channel::Source::RrdHttpStream { .. } => "http",
                re_smart_channel::Source::RrdWebEventListener { .. } => "web_event",
                re_smart_channel::Source::Sdk => "sdk", // show()
                re_smart_channel::Source::WsClient { .. } => "ws_client", // spawn()
                re_smart_channel::Source::TcpServer { .. } => "tcp_server", // connect()
            };
            self.register("data_source", data_source);
        }

        self.record(Event::append("open_recording"));
    }
}

// ----------------------------------------------------------------------------

// When analytics are disabled:
#[cfg(not(all(not(target_arch = "wasm32"), feature = "analytics")))]
impl ViewerAnalytics {
    #[allow(clippy::unused_self)]
    pub fn on_viewer_started(
        &mut self,
        _build_info: &re_build_info::BuildInfo,
        _app_env: &crate::AppEnvironment,
    ) {
    }
    #[allow(clippy::unused_self)]
    pub fn on_open_recording(&mut self, _log_db: &re_data_store::LogDb) {}
}
