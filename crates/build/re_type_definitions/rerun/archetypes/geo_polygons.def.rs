// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Filled geospatial polygons, including concave boundaries and holes.
///
/// Coordinates use [EPSG:4326](https://epsg.io/4326) latitude and longitude in North/East-positive degrees.
/// Each polygon is a separate selectable instance and may have its own color.
/// Log disjoint parts as separate polygon instances.
///
/// \example archetypes/geo_polygons_simple title="Log a polygon with a hole"
#[rerun::rerun_type]
#[docs(category = "Geospatial")]
#[docs(view_types = "MapView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "GeoPolygons")]
#[rust(derive(PartialEq))]
pub struct GeoPolygons {
    /// Polygons with exterior and interior rings.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub polygons: Vec<rerun::components::GeoPolygon>,

    /// Optional fill colors, one per polygon instance.
    ///
    /// \py Colors are RGB or RGBA in sRGB gamma-space, as 0-1 floats or 0-255 integers.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,
}
