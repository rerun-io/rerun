// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 2D map view to display geospatial primitives.
///
/// \example views/map title="Use a blueprint to create a map view." image="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "Map")]
#[rerun(state = "unstable")]
pub struct MapView {
    /// Configures the zoom level of the map view.
    pub zoom: rerun::blueprint::archetypes::MapZoom,

    /// Configuration for the background map of the map view.
    pub background: rerun::blueprint::archetypes::MapBackground,
}
