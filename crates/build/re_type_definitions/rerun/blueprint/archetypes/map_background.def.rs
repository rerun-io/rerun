// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the background map of the map view.
#[rerun::rerun_type]
#[python(aliases = "blueprint_components.MapProviderLike")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct MapBackground {
    /// Map provider and style to use.
    ///
    /// **Note**: Requires a Mapbox API key in the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable.
    #[rerun(optional)]
    pub provider: rerun::blueprint::components::MapProvider,
}
