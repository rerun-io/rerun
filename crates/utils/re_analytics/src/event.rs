// This file is linked to in a number of places, do not move/rename it without updating all the links!

//! All analytics events collected by the Rerun viewer are defined in this file.
//!
//! Analytics can be completely disabled with `rerun analytics disable`,
//! or by compiling rerun without the `analytics` feature flag.
//!
//! All collected analytics data is anonymized, stripping all personal identifiable information
//! as well as information about user data.
//! Read more about our analytics policy at <https://github.com/rerun-io/rerun/tree/main/crates/utils/re_analytics>.

use std::collections::HashMap;

use re_build_info::{BuildInfo, CrateVersion};
use url::Url;

use crate::{AnalyticsEvent, Event, EventKind, Properties, Property};

// ---------------------------------------------------------------

/// Records a crash caused by a panic.
///
/// Used in `re_crash_handler`.
pub struct CrashPanic {
    pub build_info: BuildInfo,

    /// Anonymized
    pub callstack: String,
    pub message: Option<String>,
    pub file_line: Option<String>,
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

// ---------------------------------------------------------------

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

// ---------------------------------------------------------------

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

impl Event for Identify {
    const NAME: &'static str = "$identify";

    const KIND: EventKind = EventKind::Identify;
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

// ---------------------------------------------------------------

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

    pub screen_info: ScreenInfo,
}

impl Properties for ViewerRuntimeInformation {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            is_docker,
            is_wsl,
            graphics_adapter_backend,
            re_renderer_device_tier,
            screen_info,
        } = self;

        event.insert("is_docker", is_docker);
        event.insert("is_wsl", is_wsl);
        event.insert("graphics_adapter_backend", graphics_adapter_backend);
        event.insert("re_renderer_device_tier", re_renderer_device_tier);
        screen_info.serialize(event);
    }
}

// ---------------------------------------------------------------
/// Information about the user's monitor.
pub struct ScreenInfo {
    //// zoom_factor * native_pixels_per_point
    ///
    /// Is it usually 1.0 or 2.0, but could be anything.
    pub pixels_per_point: f32,

    /// OS pixel density
    pub native_pixels_per_point: Option<f32>,

    /// Chosen zoom, with cmd +/-.
    ///
    /// Default is 1.0, but the user can change it.
    pub zoom_factor: f32,
}

impl Properties for ScreenInfo {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            pixels_per_point,
            native_pixels_per_point,
            zoom_factor,
        } = self;

        event.insert("pixels_per_point", pixels_per_point);
        event.insert_opt("native_pixels_per_point", native_pixels_per_point);
        event.insert("zoom_factor", zoom_factor);
    }
}

// -----------------------------------------------

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

impl From<Id> for Property {
    fn from(val: Id) -> Self {
        match val {
            Id::Official(id) => Self::String(id),
            Id::Hashed(id) => id,
        }
    }
}

// ----------------------------------------------------------------------------

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

// ---------------------------------------------------------------

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

// ---------------------------------------------------------------

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

// -----------------------------------------------

// -----------------------------------------------

/// Sent the first time a `?` help button is clicked.
///
/// Is used to track how many users find the help button.
pub struct HelpButtonFirstClicked {}

impl Event for HelpButtonFirstClicked {
    const NAME: &'static str = "help-button-clicked";
}

impl Properties for HelpButtonFirstClicked {
    fn serialize(self, _event: &mut AnalyticsEvent) {
        let Self {} = self;
    }
}

// -----------------------------------------------

/// The user opened the settings screen.
pub struct SettingsOpened {}

impl Event for SettingsOpened {
    const NAME: &'static str = "settings-opened";
}

impl Properties for SettingsOpened {
    fn serialize(self, _event: &mut AnalyticsEvent) {
        let Self {} = self;
    }
}

// -----------------------------------------------

/// Links the current anonymous analytics ID to an authenticated user.
///
/// This is sent when a user logs in, allowing us to connect their
/// pre-login anonymous activity with their authenticated identity.
pub struct SetPersonProperty {
    pub email: String,

    /// The user's organization ID from the JWT claims.
    pub organization_id: String,
}

impl Event for SetPersonProperty {
    const NAME: &'static str = "$set";

    const KIND: EventKind = EventKind::SetPersonProperties;
}

impl Properties for SetPersonProperty {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            email,
            organization_id,
        } = self;
        event.insert("email", email);
        event.insert("organization_id", organization_id);
    }
}

// -----------------------------------------------

/// Tracks when a data source is loaded from the viewer.
///
/// This is sent when a user opens a file, URL, or other data source.
pub struct LoadDataSource {
    /// The type of data source being loaded (e.g., "file", "http" etc.).
    pub source_type: &'static str,

    /// The file extension if applicable (e.g., "rrd", "png", "glb").
    /// None for non-file sources like stdin or gRPC streams.
    pub file_extension: Option<String>,

    /// How the file was opened (e.g., "cli", "`file_dialog`" etc.).
    /// Only applicable for file-based sources.
    pub file_source: Option<&'static str>,

    /// Whether the data source stream was started successfully.
    pub started_successfully: bool,
}

impl Event for LoadDataSource {
    const NAME: &'static str = "load_data_source";
}

impl Properties for LoadDataSource {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            source_type,
            file_extension,
            file_source,
            started_successfully,
        } = self;

        event.insert("source_type", source_type);
        event.insert_opt("file_extension", file_extension);
        event.insert_opt("file_source", file_source.map(|s| s.to_owned()));
        event.insert("started_successfully", started_successfully);
    }
}

// -----------------------------------------------

/// Tracks CLI command invocations.
///
/// This is sent when a user runs the Rerun CLI with any command.
#[derive(Default)]
pub struct CliCommandInvoked {
    /// The main command (e.g., "rrd", "auth", "mcap").
    /// "viewer" is used when no subcommand is specified.
    pub command: &'static str,

    /// The subcommand if any (e.g., "compact", "merge", "login").
    pub subcommand: Option<&'static str>,

    // --- Flags ---
    pub web_viewer: bool,
    pub serve_web: bool,
    pub serve_grpc: bool,
    pub connect: bool,
    pub save: bool,
    pub screenshot_to: bool,
    pub newest_first: bool,
    pub persist_state_disabled: bool,
    pub profile: bool,
    pub expect_data_soon: bool,
    pub hide_welcome_screen: bool,
    pub detach_process: bool,
    pub test_receive: bool,
}

impl Event for CliCommandInvoked {
    const NAME: &'static str = "cli_command_invoked";
}

impl Properties for CliCommandInvoked {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            command,
            subcommand,
            web_viewer,
            serve_web,
            serve_grpc,
            connect,
            save,
            screenshot_to,
            newest_first,
            persist_state_disabled,
            profile,
            expect_data_soon,
            hide_welcome_screen,
            detach_process,
            test_receive,
        } = self;

        event.insert("command", command);
        event.insert_opt("subcommand", subcommand.map(|s| s.to_owned()));
        event.insert("web_viewer", web_viewer);
        event.insert("serve_web", serve_web);
        event.insert("serve_grpc", serve_grpc);
        event.insert("connect", connect);
        event.insert("save", save);
        event.insert("screenshot_to", screenshot_to);
        event.insert("newest_first", newest_first);
        event.insert("persist_state_disabled", persist_state_disabled);
        event.insert("profile", profile);
        event.insert("expect_data_soon", expect_data_soon);
        event.insert("hide_welcome_screen", hide_welcome_screen);
        event.insert("detach_process", detach_process);
        event.insert("test_receive", test_receive);
    }
}

// -----------------------------------------------

/// Tracks navigation clicks on the welcome screen cards.
///
/// This event is sent when users click on cards on the welcome screen,
/// such as documentation links or cloud-related call-to-actions (CTAs).
pub struct WelcomeScreenNavigation {
    /// Type of the card. E.g. "docs", "redap".
    pub card_type: String,

    /// The destination URL that was navigated to.
    /// Empty string if the click was on a CTA that opened a modal instead.
    pub destination: String,

    /// Whether this was a click on a cloud modal CTA (e.g. "Add server", "Login").
    pub cta_cloud: bool,

    /// Whether the user is logged in.
    pub is_logged_in: bool,

    /// Whether there is a server added.
    pub has_server: bool,
}

impl Event for WelcomeScreenNavigation {
    const NAME: &'static str = "welcome_navigation";
}

impl Properties for WelcomeScreenNavigation {
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            card_type,
            destination,
            cta_cloud,
            is_logged_in,
            has_server,
        } = self;

        event.insert("card_type", card_type);
        event.insert("destination", destination);
        event.insert("cta_cloud", cta_cloud);
        event.insert("is_logged_in", is_logged_in);
        event.insert("has_server", has_server);
        event.insert("rerun_version", CrateVersion::LOCAL.to_string());
    }
}

// -----------------------------------------------

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
