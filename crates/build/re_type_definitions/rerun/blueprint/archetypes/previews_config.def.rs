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
    // TODO(RR-4810): Add shared playback state, speed, and related time settings.
}
