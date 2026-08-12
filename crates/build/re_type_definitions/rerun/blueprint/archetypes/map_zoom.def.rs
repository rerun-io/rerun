// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration of the map view zoom level.
//TODO(ab): Turn this archetype into `MapArea` and include a `center: LatLon` component or similar
#[rerun::rerun_type]
#[python(aliases = "datatypes.Float64Like")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct MapZoom {
    /// Zoom level for the map.
    ///
    /// Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).
    #[rerun(optional)]
    pub zoom: rerun::blueprint::components::ZoomLevel,
}
