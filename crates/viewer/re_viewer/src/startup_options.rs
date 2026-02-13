use crate::app_blueprint::PanelStateOverrides;
use crate::event::ViewerEventCallback;

/// Settings set once at startup (e.g. via command-line options) and not serialized.
#[derive(Clone)]
pub struct StartupOptions {
    /// When the total process RAM reaches this limit, we GC old data.
    pub memory_limit: re_memory::MemoryLimit,

    pub persist_state: bool,

    /// Whether or not the app is running in the context of a Jupyter Notebook.
    pub is_in_notebook: bool,

    /// Set to identify the web page the viewer is running on.
    #[cfg(target_arch = "wasm32")]
    pub location: Option<eframe::Location>,

    /// Take a screenshot of the app and quit.
    /// We use this to generate screenshots of our examples.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshot_to_path_then_quit: Option<std::path::PathBuf>,

    /// A user has specifically requested the welcome screen be hidden.
    pub hide_welcome_screen: bool,

    /// Detach Rerun Viewer process from the application process.
    #[cfg(not(target_arch = "wasm32"))]
    pub detach_process: bool,

    /// Set the screen resolution in logical points.
    #[cfg(not(target_arch = "wasm32"))]
    pub resolution_in_points: Option<[f32; 2]>,

    /// This is a hint that we expect a recording to stream in very soon.
    ///
    /// This is set by the `spawn()` method in our logging SDK.
    ///
    /// The viewer will respond by fading in the welcome screen,
    /// instead of showing it directly.
    /// This ensures that it won't blink for a few frames before switching to the recording.
    pub expect_data_soon: Option<bool>,

    /// Forces wgpu backend to use the specified graphics API, e.g. `webgl` or `webgpu`.
    pub force_wgpu_backend: Option<String>,

    /// Overwrites hardware acceleration option for video decoding.
    ///
    /// By default uses the last provided setting, which is `auto` if never configured.
    /// This also can be changed in the viewer's option menu.
    pub video_decoder_hw_acceleration: Option<re_video::DecodeHardwareAcceleration>,

    /// External interactions with the Viewer host (JS, custom egui app, notebook, etc.).
    pub on_event: Option<ViewerEventCallback>,

    /// Fullscreen is handled by JS on web.
    ///
    /// This holds some callbacks which we use to communicate
    /// about fullscreen state to JS.
    #[cfg(target_arch = "wasm32")]
    pub fullscreen_options: Option<crate::web::FullscreenOptions>,

    /// Default overrides for state of top/side/bottom panels.
    pub panel_state_overrides: PanelStateOverrides,

    /// Whether or not to enable usage of the `History` API on web.
    ///
    /// It is disabled by default.
    ///
    /// This should only be enabled when it is acceptable for `rerun`
    /// to push its own entries into browser history.
    ///
    /// That only makes sense if it has "taken over" a page, and is
    /// the only thing on that page. If you are embedding multiple
    /// viewers onto the same page, then it's better to turn this off.
    ///
    /// We use browser history in a limited way to track the currently
    /// open example or redap recording, see [`crate::history`].
    #[cfg(target_arch = "wasm32")]
    pub enable_history: bool,

    /// The base viewer url that's used when sharing a link in this viewer.
    ///
    /// If not set:
    /// * notebooks & native: use rerun.io/viewer with the crate's last known stable version
    /// * web viewers: use the url of the page it is embedded in
    pub viewer_base_url: Option<String>,
}

impl StartupOptions {
    /// Returns `StartupOptions::enable_history` on web, and `false` on native.
    #[allow(clippy::allow_attributes, clippy::unused_self)] // Only used on web.
    pub fn web_history_enabled(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.enable_history
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
    }

    /// The url to use for the web viewer when sharing links.
    #[allow(clippy::allow_attributes, clippy::unused_self)] // Only used on web.
    pub fn web_viewer_base_url(&self) -> Option<url::Url> {
        // TODO(RR-1878): Would be great to grab this from the Data Platform when available.

        if let Some(url) = &self.viewer_base_url
            && let Ok(url) = url.parse::<url::Url>()
        {
            Some(url)
        } else if self.is_in_notebook || cfg!(not(target_arch = "wasm32")) {
            // Notebooks behave like native viewers here because just like on native,
            // there's no useful base url in the address bar to use.
            let version = re_build_info::build_info!().version.latest_stable();

            url::Url::parse(&format!("https://rerun.io/viewer/version/{version}")).ok()
        } else {
            #[cfg(target_arch = "wasm32")]
            {
                crate::web_tools::current_base_url().ok()
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                None
            }
        }
    }
}

impl Default for StartupOptions {
    fn default() -> Self {
        Self {
            memory_limit: re_memory::MemoryLimit::from_fraction_of_total(0.75),
            persist_state: true,
            is_in_notebook: false,

            #[cfg(target_arch = "wasm32")]
            location: None,

            #[cfg(not(target_arch = "wasm32"))]
            screenshot_to_path_then_quit: None,

            hide_welcome_screen: false,

            #[cfg(not(target_arch = "wasm32"))]
            detach_process: true,

            #[cfg(not(target_arch = "wasm32"))]
            resolution_in_points: None,

            expect_data_soon: None,
            force_wgpu_backend: None,
            video_decoder_hw_acceleration: None,

            on_event: None,

            #[cfg(target_arch = "wasm32")]
            fullscreen_options: Default::default(),

            panel_state_overrides: Default::default(),

            #[cfg(target_arch = "wasm32")]
            enable_history: false,

            viewer_base_url: None,
        }
    }
}
