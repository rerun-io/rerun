//! Analytics events sent from the viewer.
//!
//! Note that this file does not contain everything we track,
//! there are a few events defined in other crates.
//!
//! To find all of them,

use crate::AppEnvironment;
use re_analytics::Config;
use re_analytics::Properties;
use re_analytics::Property;
use re_analytics::{Event, EventKind};
use std::collections::HashMap;

pub struct Identify {
    build_info: re_build_info::BuildInfo,

    // If we happen to know the Python or Rust version used on the _host machine_, i.e. the
    // machine running the viewer, then override the versions from `build_info`.
    //
    // The Python/Rust versions appearing in user profiles always apply to the host
    // environment, _not_ the environment in which the data logging is taking place!
    rust_version: Option<String>,
    llvm_version: Option<String>,
    python_version: Option<String>,

    /// Opt-in meta-data you can set via `rerun analytics`.
    ///
    /// For instance, Rerun employees are encouraged to set `rerun analytics email`.
    /// For real users, this is usually empty.
    opt_in_metadata: HashMap<String, Property>,
}

pub struct ViewerStarted {
    /// The URL on which the web viewer is running.
    ///
    /// We _only_ collect this on `rerun.io` domains.
    url: Option<String>,

    /// The environment in which the viewer is running.
    app_env: &'static str,
}

/// Sent when a new recording is opened in the viewer.
pub struct OpenRecording {
    /// The URL on which the web viewer is running.
    ///
    /// We _only_ collect this on `rerun.io` domains.
    url: Option<String>,

    /// The environment in which the viewer is running.
    app_env: &'static str,

    store_info: Option<StoreInfo>,

    /// How data is being loaded into the viewer.
    data_source: Option<&'static str>,
}

/// Basic information about the data store.
struct StoreInfo {
    application_id: Id,
    recording_id: Id,

    /// Where data is being logged.
    store_source: String,

    rust_version: Option<String>,
    llvm_version: Option<String>,
    python_version: Option<String>,

    /// Whether or not the data is coming from one of the Rerun example applications.
    is_official_example: bool,
    app_id_starts_with_rerun_example: bool,
}

#[derive(Clone)]
enum Id {
    /// When running an example application we record the full id.
    Official(String),

    /// For user applications we hash the id.
    Hashed(Property),
}

// ----------------------------------------------------------------------------

impl From<Id> for Property {
    fn from(val: Id) -> Self {
        match val {
            Id::Official(id) => Property::String(id),
            Id::Hashed(id) => id,
        }
    }
}

impl Identify {
    pub fn new(
        config: &Config,
        build_info: re_build_info::BuildInfo,
        app_env: &AppEnvironment,
    ) -> Self {
        let (rust_version, llvm_version, python_version) = match app_env {
            AppEnvironment::RustSdk {
                rustc_version,
                llvm_version,
            }
            | AppEnvironment::RerunCli {
                rustc_version,
                llvm_version,
            } => (Some(rustc_version), Some(llvm_version), None),
            AppEnvironment::PythonSdk(version) => (None, None, Some(version.to_string())),
            _ => (None, None, None),
        };

        Self {
            build_info,
            rust_version: rust_version.map(|s| s.to_owned()),
            llvm_version: llvm_version.map(|s| s.to_owned()),
            python_version,
            opt_in_metadata: config.opt_in_metadata.clone(),
        }
    }
}

impl Event for Identify {
    const NAME: &'static str = "$identify";

    const KIND: EventKind = EventKind::Update;
}

impl Properties for Identify {
    fn serialize(&self, event: &mut re_analytics::AnalyticsEvent) {
        self.build_info.serialize(event);
        event.insert_opt("rust_version", self.rust_version.clone());
        event.insert_opt("llvm_version", self.llvm_version.clone());
        event.insert_opt("python_version", self.python_version.clone());
        for (name, value) in &self.opt_in_metadata {
            event.insert(name.clone(), value.clone());
        }
    }
}

impl ViewerStarted {
    pub fn new(app_env: &AppEnvironment) -> Self {
        Self {
            url: app_env.url().cloned(),
            app_env: app_env.name(),
        }
    }
}

impl Event for ViewerStarted {
    const NAME: &'static str = "viewer_started";
}

impl Properties for ViewerStarted {
    fn serialize(&self, event: &mut re_analytics::AnalyticsEvent) {
        event.insert("app_env", self.app_env);
        event.insert_opt("url", self.url.clone());
    }
}

impl OpenRecording {
    pub fn new(app_env: &AppEnvironment, entity_db: &re_entity_db::EntityDb) -> Self {
        let store_info = entity_db.store_info().map(|store_info| {
            let application_id = if store_info.is_official_example {
                Id::Official(store_info.application_id.0.clone())
            } else {
                Id::Hashed(Property::from(store_info.application_id.0.clone()).hashed())
            };

            let recording_id = if store_info.is_official_example {
                Id::Official(store_info.store_id.to_string())
            } else {
                Id::Hashed(Property::from(store_info.store_id.to_string()).hashed())
            };

            use re_log_types::StoreSource as S;
            let store_source = match &store_info.store_source {
                S::Unknown => "unknown".to_owned(),
                S::CSdk => "c_sdk".to_owned(),
                S::PythonSdk(_version) => "python_sdk".to_owned(),
                S::RustSdk { .. } => "rust_sdk".to_owned(),
                S::File { file_source } => match file_source {
                    re_log_types::FileSource::Cli => "file_cli".to_owned(),
                    re_log_types::FileSource::DragAndDrop => "file_drag_and_drop".to_owned(),
                    re_log_types::FileSource::FileDialog => "file_dialog".to_owned(),
                },
                S::Viewer => "viewer".to_owned(),
                S::Other(other) => other.clone(),
            };

            // `rust+llvm` and `python` versions are mutually exclusive
            let mut rust_version = None;
            let mut llvm_version = None;
            let mut python_version = None;
            match &store_info.store_source {
                S::File { .. } => {
                    rust_version = Some(env!("RE_BUILD_RUSTC_VERSION").to_owned());
                    llvm_version = Some(env!("RE_BUILD_LLVM_VERSION").to_owned());
                }
                S::RustSdk {
                    rustc_version: rustc,
                    llvm_version: llvm,
                } => {
                    rust_version = Some(rustc.to_string());
                    llvm_version = Some(llvm.to_string());
                }
                S::PythonSdk(version) => {
                    python_version = Some(version.to_string());
                }
                // TODO(andreas): Send C SDK version and set it.
                S::CSdk | S::Unknown | S::Viewer | S::Other(_) => {}
            }

            let is_official_example = store_info.is_official_example;
            let app_id_starts_with_rerun_example = store_info
                .application_id
                .as_str()
                .starts_with("rerun_example");

            StoreInfo {
                application_id,
                recording_id,
                store_source,
                rust_version,
                llvm_version,
                python_version,
                is_official_example,
                app_id_starts_with_rerun_example,
            }
        });

        let data_source = entity_db.data_source.as_ref().map(|v| match v {
            re_smart_channel::SmartChannelSource::File(_) => "file", // .rrd, .png, .glb, …
            re_smart_channel::SmartChannelSource::RrdHttpStream { .. } => "http",
            re_smart_channel::SmartChannelSource::RrdWebEventListener { .. } => "web_event",
            re_smart_channel::SmartChannelSource::Sdk => "sdk", // show()
            re_smart_channel::SmartChannelSource::WsClient { .. } => "ws_client", // spawn()
            re_smart_channel::SmartChannelSource::TcpServer { .. } => "tcp_server", // connect()
            re_smart_channel::SmartChannelSource::Stdin => "stdin",
        });

        Self {
            url: app_env.url().cloned(),
            app_env: app_env.name(),
            store_info,
            data_source,
        }
    }
}

impl Event for OpenRecording {
    const NAME: &'static str = "open_recording";
}

impl Properties for OpenRecording {
    fn serialize(&self, event: &mut re_analytics::AnalyticsEvent) {
        event.insert_opt("url", self.url.clone());
        event.insert("app_env", self.app_env);

        if let Some(store_info) = &self.store_info {
            event.insert("application_id", store_info.application_id.clone());
            event.insert("recording_id", store_info.recording_id.clone());
            event.insert("store_source", store_info.store_source.clone());
            event.insert_opt("rust_version", store_info.rust_version.clone());
            event.insert_opt("llvm_version", store_info.llvm_version.clone());
            event.insert_opt("python_version", store_info.python_version.clone());
            event.insert("is_official_example", store_info.is_official_example);
            event.insert(
                "app_id_starts_with_rerun_example",
                store_info.app_id_starts_with_rerun_example,
            );
        }

        if let Some(data_source) = self.data_source {
            event.insert("data_source", data_source);
        }
    }
}
