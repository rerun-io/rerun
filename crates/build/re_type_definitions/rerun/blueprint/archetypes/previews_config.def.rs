// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Shared configuration for every preview cell in a table blueprint.
///
/// This archetype is stored at the entity `/table` alongside [`rerun::blueprint::archetypes::TableBlueprint`].
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct PreviewsConfig {
    /// The timeline used by every preview cell.
    ///
    /// If left empty a timeline is automatically picked, preferring custom over built-in.
    #[rerun(optional)]
    pub timeline: Option<rerun::blueprint::components::TimelineName>,

    /// Whether every preview cell plays.
    ///
    /// A single preview can still be played or paused on its own.
    /// Defaults to paused, and following plays like playing.
    #[rerun(optional)]
    pub play_state: Option<rerun::blueprint::components::PlayState>,

    /// Playback speed of every preview cell.
    ///
    /// Defaults to 1.0.
    #[rerun(optional)]
    pub playback_speed: Option<rerun::blueprint::components::PlaybackSpeed>,

    /// Where every preview cell starts, as a fraction of the recording's time range.
    ///
    /// 0.0 is the start and 1.0 the end.
    /// Defaults to 0.5.
    #[rerun(optional)]
    pub start_point: Option<rerun::blueprint::components::TimelineFraction>,
}
