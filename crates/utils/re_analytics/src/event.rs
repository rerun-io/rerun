// This file is linked to in a number of places, do not move/rename it without updating all the links!

//! All analytics events collected by the Rerun viewer are defined in this file.
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.
//!
//! All collected analytics data is anonymized, stripping all personal identifiable information
//! as well as information about user data.
//! Read more about our analytics policy at <https://github.com/rerun-io/rerun/tree/main/crates/utils/re_analytics>.

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
    /// This will be used to populate `hashed_root_domain` property for all urls.
    /// This will also populate `rerun_url` property if the url root domain is `rerun.io`.
    pub url: Option<String>,

    /// The environment in which the viewer is running.
    pub app_env: &'static str,

    /// Sparse information about the runtime environment the viewer is running in.
    pub runtime_info: ViewerRuntimeInformation,
}

/// Some sparse information about the runtime environment the viewer is running in.
pub struct ViewerRuntimeInformation {
    /// Does it look like the viewer is running inside a Docker container?
    pub is_docker: bool,

    /// Whether the viewer is started directly from within Windows Subsystem for Linux (WSL).
    pub is_wsl: bool,

    /// The wgpu graphics backend used by the viewer.
    ///
    /// For possible values see [`wgpu::Backend`](https://docs.rs/wgpu/latest/wgpu/enum.Backend.html).
    pub graphics_adapter_backend: String,

    /// The device tier `re_renderer` identified for the graphics adapter.
    ///
    /// For possible values see [`re_renderer::config::DeviceTier`](https://docs.rs/re_renderer/latest/re_renderer/config/enum.DeviceTier.html).
    /// This is a very rough indication of the capabilities of the graphics adapter.
    /// We do not want to send details graphics driver/capability information here since
    /// it's too detailed (could be used for fingerprinting which we don't want) and not as useful
    /// anyways since it's hard to learn about the typically identified capabilities.
    pub re_renderer_device_tier: String,
}

impl Properties for ViewerRuntimeInformation {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            is_docker,
            is_wsl,
            graphics_adapter_backend,
            re_renderer_device_tier,
        } = self;

        event.insert("is_docker", is_docker);
        event.insert("is_wsl", is_wsl);
        event.insert("graphics_adapter_backend", graphics_adapter_backend);
        event.insert("re_renderer_device_tier", re_renderer_device_tier);
    }
}

/// Sent when a new recording is opened.
///
/// Used in `re_viewer`.
pub struct OpenRecording {
    /// The URL on which the web viewer is running.
    ///
    /// This will be used to populate `hashed_root_domain` property for all urls.
    /// This will also populate `rerun_url` property if the url root domain is `rerun.io`.
    pub url: Option<String>,

    /// The environment in which the viewer is running.
    pub app_env: &'static str,

    pub store_info: Option<StoreInfo>,

    /// How data is being loaded into the viewer.
    pub data_source: Option<&'static str>,
}

/// Basic information about a recording's chunk store.
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

    /// The Rerun version that was used to encode the RRD data.
    pub store_version: String,

    // Various versions of the host environment.
    pub rust_version: Option<String>,
    pub llvm_version: Option<String>,
    pub python_version: Option<String>,

    // Whether or not the data is coming from one of the Rerun example applications.
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
use url::Url;

use crate::{AnalyticsEvent, Event, EventKind, Properties, Property};

impl From<Id> for Property {
    fn from(val: Id) -> Self {
        match val {
            Id::Official(id) => Self::String(id),
            Id::Hashed(id) => id,
        }
    }
}

impl Event for Identify {
    const NAME: &'static str = "$identify";

    const KIND: EventKind = EventKind::Update;
}

impl Properties for Identify {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            build_info,
            rust_version,
            llvm_version,
            python_version,
            opt_in_metadata,
        } = self;

        build_info.serialize(event);
        event.insert_opt("rust_version", rust_version);
        event.insert_opt("llvm_version", llvm_version);
        event.insert_opt("python_version", python_version);
        for (name, value) in opt_in_metadata {
            event.insert(name, value);
        }
    }
}

impl Event for ViewerStarted {
    const NAME: &'static str = "viewer_started";
}

const RERUN_DOMAINS: [&str; 1] = ["rerun.io"];

/// Given a URL, extract the root domain.
fn extract_root_domain(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let domain = parsed.domain()?;
    let parts = domain.split('.').collect::<Vec<_>>();
    if parts.len() >= 2 {
        Some(parts[parts.len() - 2..].join("."))
    } else {
        None
    }
}

fn add_sanitized_url_properties(event: &mut AnalyticsEvent, url: Option<String>) {
    let Some(root_domain) = url.as_ref().and_then(|url| extract_root_domain(url)) else {
        return;
    };

    if RERUN_DOMAINS.contains(&root_domain.as_str()) {
        event.insert_opt("rerun_url", url);
    }

    let hashed = Property::from(root_domain).hashed();
    event.insert("hashed_root_domain", hashed);
}

impl Properties for ViewerStarted {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            url,
            app_env,
            runtime_info,
        } = self;

        event.insert("app_env", app_env);
        add_sanitized_url_properties(event, url);
        runtime_info.serialize(event);
    }
}

impl Event for OpenRecording {
    const NAME: &'static str = "open_recording";
}

impl Properties for OpenRecording {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            url,
            app_env,
            store_info,
            data_source,
        } = self;

        add_sanitized_url_properties(event, url);

        event.insert("app_env", app_env);

        if let Some(store_info) = store_info {
            let StoreInfo {
                application_id,
                recording_id,
                store_source,
                store_version,
                rust_version,
                llvm_version,
                python_version,

                app_id_starts_with_rerun_example,
            } = store_info;

            event.insert("application_id", application_id);
            event.insert("recording_id", recording_id);
            event.insert("store_source", store_source);
            event.insert("store_version", store_version);
            event.insert_opt("rust_version", rust_version);
            event.insert_opt("llvm_version", llvm_version);
            event.insert_opt("python_version", python_version);
            event.insert(
                "app_id_starts_with_rerun_example",
                app_id_starts_with_rerun_example,
            );
        }

        if let Some(data_source) = data_source {
            event.insert("data_source", data_source);
        }
    }
}

impl Event for CrashPanic {
    const NAME: &'static str = "crash-panic";
}

impl Properties for CrashPanic {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            build_info,
            callstack,
            message,
            file_line,
        } = self;

        build_info.serialize(event);
        event.insert("callstack", callstack);
        event.insert_opt("message", message);
        event.insert_opt("file_line", file_line);
    }
}

pub struct CrashSignal {
    pub build_info: BuildInfo,
    pub signal: String,
    pub callstack: String,
}

impl Event for CrashSignal {
    const NAME: &'static str = "crash-signal";
}

impl Properties for CrashSignal {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            build_info,
            signal,
            callstack,
        } = self;

        build_info.serialize(event);
        event.insert("signal", signal.clone());
        event.insert("callstack", callstack.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_domain() {
        // Valid urls
        assert_eq!(
            extract_root_domain("https://rerun.io"),
            Some("rerun.io".to_owned())
        );
        assert_eq!(
            extract_root_domain("https://ReRun.io"),
            Some("rerun.io".to_owned())
        );
        assert_eq!(
            extract_root_domain("http://app.rerun.io"),
            Some("rerun.io".to_owned())
        );
        assert_eq!(
            extract_root_domain(
                "https://www.rerun.io/viewer?url=https://app.rerun.io/version/0.15.1/examples/detect_and_track_objects.rrd"
            ),
            Some("rerun.io".to_owned())
        );

        // Local domains
        assert_eq!(
            extract_root_domain("http://localhost:9090/?url=rerun%2Bhttp://localhost:9877"),
            None
        );
        assert_eq!(
            extract_root_domain("http://127.0.0.1:9090/?url=rerun%2Bhttp://localhost:9877"),
            None
        );

        // Invalid urls
        assert_eq!(extract_root_domain("rerun.io"), None);
        assert_eq!(extract_root_domain("https:/rerun"), None);
        assert_eq!(extract_root_domain("https://rerun"), None);
    }
}
