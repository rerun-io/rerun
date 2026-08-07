// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Shared state for the 3 collapsible panels.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct PanelBlueprint {
    /// Current state of the panel.
    #[rerun(optional)]
    pub state: Option<rerun::blueprint::components::PanelState>,
    // TODO(jleibs): Add a float to track how expanded the panel is.
}

/// Time panel specific state.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct TimePanelBlueprint {
    /// Current state of the panel.
    #[rerun(optional)]
    pub state: Option<rerun::blueprint::components::PanelState>,

    /// What timeline the panel is on.
    #[rerun(optional)]
    pub timeline: Option<rerun::blueprint::components::TimelineName>,

    /// A time playback speed multiplier.
    #[rerun(optional)]
    pub playback_speed: Option<rerun::blueprint::components::PlaybackSpeed>,

    /// Frames per second. Only applicable for sequence timelines.
    #[rerun(optional)]
    pub fps: Option<rerun::blueprint::components::Fps>,

    /// If the time is currently paused, playing, or following.
    ///
    /// Defaults to either playing or following, depending on the data source.
    #[rerun(optional)]
    pub play_state: Option<rerun::blueprint::components::PlayState>,

    /// How the time should loop. A selection loop only works if there is also a `time_selection` passed.
    ///
    /// Defaults to off.
    #[rerun(optional)]
    pub loop_mode: Option<rerun::blueprint::components::LoopMode>,

    /// Selects a range of time on the time panel.
    #[rerun(optional)]
    pub time_selection: Option<rerun::blueprint::components::AbsoluteTimeRange>,
}
