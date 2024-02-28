//! All telemetry analytics collected by the Rerun Viewer are defined in this file for easy auditing.
//!
//! There are two exceptions:
//! * `crates/rerun/src/crash_handler.rs` sends anonymized callstacks on crashes
//! * `crates/re_web_viewer_server/src/lib.rs` sends an anonymous event when a `.wasm` web-viewer is served.
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.
//!
//! DO NOT MOVE THIS FILE without updating all the docs pointing to it!

#[cfg(feature = "analytics")]
use re_analytics::{Analytics, Event, Property};

use crate::StartupOptions;

pub struct ViewerAnalytics {
    // NOTE: Optional because it is possible to have the `analytics` feature flag enabled
    // while at the same time opting-out of analytics at run-time.
    #[cfg(feature = "analytics")]
    analytics: Option<Analytics>,
}

#[cfg(feature = "analytics")]
impl ViewerAnalytics {
    #[allow(unused_mut, clippy::let_and_return)]
    pub fn new(startup_options: &StartupOptions) -> Self {
        re_tracing::profile_function!();

        // We only want to have analytics on `*.rerun.io`,
        // so we early-out if we detect we're running in a notebook.
        if startup_options.is_in_notebook {
            return Self { analytics: None };
        }

        let analytics = match Analytics::new(std::time::Duration::from_secs(2)) {
            Ok(analytics) => Some(analytics),
            Err(err) => {
                re_log::error!(%err, "failed to initialize analytics SDK");
                None
            }
        };

        let mut analytics = Self { analytics };

        // We only want to send `url` if we're on a `rerun.io` domain.
        #[cfg(target_arch = "wasm32")]
        if let Some(location) = startup_options.location.as_ref() {
            if location.hostname == "rerun.io" || location.hostname.ends_with(".rerun.io") {
                analytics.register("url", location.url.clone());
            }
        }

        analytics
    }

    fn record(&self, event: Event) {
        if let Some(analytics) = &self.analytics {
            analytics.record(event);
        }
    }

    /// Register a property that will be included in all append-events.
    fn register(&mut self, name: &'static str, property: impl Into<Property>) {
        if let Some(analytics) = &mut self.analytics {
            analytics.register_append_property(name, property);
        }
    }

    /// Deregister a property.
    fn deregister(&mut self, name: &'static str) {
        if let Some(analytics) = &mut self.analytics {
            analytics.deregister_append_property(name);
        }
    }
}

// ----------------------------------------------------------------------------

// TODO(jan): add URL (iff domain is `rerun.io`)
// TODO(jan): make sure analytics knows the event comes from web

/// Here follows all the analytics collected by the Rerun Viewer.
#[cfg(feature = "analytics")]
impl ViewerAnalytics {
    /// When the viewer is first started
    pub fn on_viewer_started(
        &mut self,
        build_info: &re_build_info::BuildInfo,
        app_env: &crate::AppEnvironment,
    ) {
        re_tracing::profile_function!();
        use crate::AppEnvironment;
        let app_env_str = match app_env {
            AppEnvironment::CSdk => "c_sdk",
            AppEnvironment::PythonSdk(_) => "python_sdk",
            AppEnvironment::RustSdk { .. } => "rust_sdk",
            AppEnvironment::RerunCli { .. } => "rerun_cli",
            AppEnvironment::Web => "web_viewer",
            AppEnvironment::Custom(_) => "custom",
        };
        self.register("app_env", app_env_str);

        #[cfg(feature = "analytics")]
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
            if let AppEnvironment::PythonSdk(version) = app_env {
                event = event.with_prop("python_version", version.to_string());
            }

            // Append opt-in metadata.
            // In practice this is the email of Rerun employees
            // who register their emails with `rerun analytics email`.
            // This is how we filter out employees from actual users!
            for (name, value) in &analytics.config().opt_in_metadata {
                event = event.with_prop(name.clone(), value.clone());
            }

            analytics.record(event);
        }

        self.record(Event::append("viewer_started"));
    }

    /// When we have loaded the start of a new recording.
    pub fn on_open_recording(&mut self, entity_db: &re_entity_db::EntityDb) {
        use re_log_types::StoreSource;

        if entity_db.store_kind() != re_log_types::StoreKind::Recording {
            return;
        }

        if let Some(store_info) = entity_db.store_info() {
            // We hash the application_id and recording_id unless this is an official example.
            // That's because we want to be able to track which are the popular examples,
            // but we don't want to collect actual application ids.
            self.register("application_id", {
                let prop = Property::from(store_info.application_id.0.clone());
                if store_info.is_official_example {
                    prop
                } else {
                    prop.hashed()
                }
            });
            self.register("recording_id", {
                let prop = Property::from(store_info.store_id.to_string());
                if store_info.is_official_example {
                    prop
                } else {
                    prop.hashed()
                }
            });

            let store_source = match &store_info.store_source {
                StoreSource::Unknown => "unknown".to_owned(),
                StoreSource::CSdk => "c_sdk".to_owned(),
                StoreSource::PythonSdk(_version) => "python_sdk".to_owned(),
                StoreSource::RustSdk { .. } => "rust_sdk".to_owned(),
                StoreSource::File { file_source } => match file_source {
                    re_log_types::FileSource::Cli => "file_cli".to_owned(),
                    re_log_types::FileSource::DragAndDrop => "file_drag_and_drop".to_owned(),
                    re_log_types::FileSource::FileDialog => "file_dialog".to_owned(),
                    re_log_types::FileSource::Sdk => "file_sdk".to_owned(),
                },
                StoreSource::Viewer => "viewer".to_owned(),
                StoreSource::Other(other) => other.clone(),
            };

            // If we happen to know the Python or Rust version used on the _recording machine_,
            // then append it to all future events.
            //
            // The Python/Rust versions appearing in events always apply to the recording
            // environment, _not_ the environment in which the viewer is running!
            #[allow(clippy::match_same_arms)]
            match &store_info.store_source {
                StoreSource::File { .. } => {
                    self.register("rust_version", env!("RE_BUILD_RUSTC_VERSION")); // Rust/LLVM version used to compile the viewer
                    self.register("llvm_version", env!("RE_BUILD_LLVM_VERSION")); // Rust/LLVM version used to compile the viewer
                    self.deregister("python_version"); // can't be both!
                }
                StoreSource::RustSdk {
                    rustc_version,
                    llvm_version,
                } => {
                    self.register("rust_version", rustc_version.to_string()); // Rust/LLVM version of the code compiling the Rust SDK
                    self.register("llvm_version", llvm_version.to_string()); // Rust/LLVM version of the code compiling the Rust SDK
                    self.deregister("python_version"); // can't be both!
                }
                StoreSource::PythonSdk(version) => {
                    self.register("python_version", version.to_string());
                    self.deregister("rust_version"); // can't be both!
                    self.deregister("llvm_version"); // can't be both!
                }
                StoreSource::CSdk => {} // TODO(andreas): Send version and set it.
                StoreSource::Unknown | StoreSource::Viewer | StoreSource::Other(_) => {}
            }

            self.register("store_source", store_source);
            self.register("is_official_example", store_info.is_official_example);
            self.register(
                "app_id_starts_with_rerun_example",
                store_info
                    .application_id
                    .as_str()
                    .starts_with("rerun_example"),
            );
        }

        if let Some(data_source) = &entity_db.data_source {
            let data_source = match data_source {
                re_smart_channel::SmartChannelSource::File(_) => "file", // .rrd, .png, .glb, …
                re_smart_channel::SmartChannelSource::RrdHttpStream { .. } => "http",
                re_smart_channel::SmartChannelSource::RrdWebEventListener { .. } => "web_event",
                re_smart_channel::SmartChannelSource::Sdk => "sdk", // show()
                re_smart_channel::SmartChannelSource::WsClient { .. } => "ws_client", // spawn()
                re_smart_channel::SmartChannelSource::TcpServer { .. } => "tcp_server", // connect()
                re_smart_channel::SmartChannelSource::Stdin => "stdin",
            };
            self.register("data_source", data_source);
        }

        self.record(Event::append("open_recording"));
    }
}

#[cfg(not(feature = "analytics"))]
impl ViewerAnalytics {
    pub fn new(_startup_options: &StartupOptions) -> Self {
        Self {}
    }

    #[allow(clippy::unused_self)]
    pub fn on_viewer_started(
        &mut self,
        _build_info: &re_build_info::BuildInfo,
        _app_env: &crate::AppEnvironment,
    ) {
    }

    #[allow(clippy::unused_self)]
    pub fn on_open_recording(&mut self, _entity_db: &re_entity_db::EntityDb) {}
}
