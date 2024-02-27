// This file is linked to in a number of places, do not move/rename it without updating all the links!

//! All analytics events collected by the Rerun viewer are defined in this file.
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.
//!
//! All collected analytics data is anonymized, stripping all personal identifiable information
//! as well as information about user data.
//! Read more about our analytics policy at <https://github.com/rerun-io/rerun/tree/main/crates/re_analytics>.

/// Records a crash caused by a panic.
///
/// Used in `re_crash_handler`.
pub struct CrashPanic {
    pub build_info: BuildInfo,
    pub callstack: String,
    pub message: Option<String>,
    pub file_line: Option<String>,
}

/// Holds information about the user's environment.
///
/// Used in `re_viewer`.
pub struct Identify {
    /// Info on how the `re_viewer` crate was built.
    pub build_info: re_build_info::BuildInfo,

    // If we happen to know the Python or Rust version used on the _host machine_, i.e. the
    // machine running the viewer, then override the versions from `build_info`.
    //
    // The Python/Rust versions appearing in user profiles always apply to the host
    // environment, _not_ the environment in which the data logging is taking place!
    pub rust_version: Option<String>,
    pub llvm_version: Option<String>,
    pub python_version: Option<String>,

    /// Opt-in meta-data you can set via `rerun analytics`.
    ///
    /// For instance, Rerun employees are encouraged to set `rerun analytics email`.
    /// For real users, this is usually empty.
    pub opt_in_metadata: HashMap<String, Property>,
}

/// Sent when the viewer is first started.
///
/// Used in `re_viewer`.
pub struct ViewerStarted {
    /// The URL on which the web viewer is running.
    ///
    /// We _only_ collect this on `rerun.io` domains.
    pub url: Option<String>,

    /// The environment in which the viewer is running.
    pub app_env: &'static str,
}

/// Sent when a new recording is opened.
///
/// Used in `re_viewer`.
pub struct OpenRecording {
    /// The URL on which the web viewer is running.
    ///
    /// We _only_ collect this on `rerun.io` domains.
    pub url: Option<String>,

    /// The environment in which the viewer is running.
    pub app_env: &'static str,

    pub store_info: Option<StoreInfo>,

    /// How data is being loaded into the viewer.
    pub data_source: Option<&'static str>,
}

/// Basic information about a recording's data store.
pub struct StoreInfo {
    /// Name of the application.
    ///
    /// In case the recording does not come from an official example, the id is hashed.
    pub application_id: Id,

    /// Name of the recording.
    ///
    /// In case the recording does not come from an official example, the id is hashed.
    pub recording_id: Id,

    /// Where data is being logged.
    pub store_source: String,

    // Various versions of the host environment.
    pub rust_version: Option<String>,
    pub llvm_version: Option<String>,
    pub python_version: Option<String>,

    // Whether or not the data is coming from one of the Rerun example applications.
    pub is_official_example: bool,
    pub app_id_starts_with_rerun_example: bool,
}

#[derive(Clone)]
pub enum Id {
    /// When running an example application we record the full id.
    Official(String),

    /// For user applications we hash the id.
    Hashed(Property),
}

/// Sent when a Wasm file is served.
///
/// Used in `re_web_viewer_server`.
pub struct ServeWasm;

impl Event for ServeWasm {
    const NAME: &'static str = "serve_wasm";
}

impl Properties for ServeWasm {
    // No properties.
}

// ----------------------------------------------------------------------------

use std::collections::HashMap;

use re_build_info::BuildInfo;

use crate::{AnalyticsEvent, Event, EventKind, Properties, Property};

impl From<Id> for Property {
    fn from(val: Id) -> Self {
        match val {
            Id::Official(id) => Property::String(id),
            Id::Hashed(id) => id,
        }
    }
}

impl Event for Identify {
    const NAME: &'static str = "$identify";

    const KIND: EventKind = EventKind::Update;
}

impl Properties for Identify {
    fn serialize(&self, event: &mut AnalyticsEvent) {
        let Self {
            build_info,
            rust_version,
            llvm_version,
            python_version,
            opt_in_metadata,
        } = self;

        build_info.serialize(event);
        event.insert_opt("rust_version", rust_version.clone());
        event.insert_opt("llvm_version", llvm_version.clone());
        event.insert_opt("python_version", python_version.clone());
        for (name, value) in opt_in_metadata {
            event.insert(name.clone(), value.clone());
        }
    }
}

impl Event for ViewerStarted {
    const NAME: &'static str = "viewer_started";
}

impl Properties for ViewerStarted {
    fn serialize(&self, event: &mut AnalyticsEvent) {
        let Self { url, app_env } = self;
        event.insert("app_env", *app_env);
        event.insert_opt("url", url.clone());
    }
}

impl Event for OpenRecording {
    const NAME: &'static str = "open_recording";
}

impl Properties for OpenRecording {
    fn serialize(&self, event: &mut AnalyticsEvent) {
        let Self {
            url,
            app_env,
            store_info,
            data_source,
        } = self;

        event.insert_opt("url", url.clone());
        event.insert("app_env", *app_env);

        if let Some(store_info) = store_info {
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

        if let Some(data_source) = *data_source {
            event.insert("data_source", data_source);
        }
    }
}

impl Event for CrashPanic {
    const NAME: &'static str = "crash-panic";
}

impl Properties for CrashPanic {
    fn serialize(&self, event: &mut AnalyticsEvent) {
        let Self {
            build_info,
            callstack,
            message,
            file_line,
        } = self;

        build_info.serialize(event);
        event.insert("callstack", callstack.clone());
        if let Some(message) = &message {
            event.insert("message", message.clone());
        }
        if let Some(file_line) = &file_line {
            event.insert("file_line", file_line.clone());
        }
    }
}
