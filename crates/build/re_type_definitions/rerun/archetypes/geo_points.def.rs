// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Geospatial points with positions expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees), and optional colors and radii.
///
/// \example archetypes/geo_points_simple title="Log a geospatial point" image="https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Geospatial")]
#[docs(view_types = "MapView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "GeoPoints")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct GeoPoints {
    /// The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
    #[rerun(required)]
    pub positions: Vec<rerun::components::LatLon>,

    /// Optional radii for the points, effectively turning them into circles.
    ///
    /// *Note*: scene units radiii are interpreted as meters.
    #[rerun(recommended)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the points.
    ///
    /// \py The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    /// \py As either 0-1 floats or 0-255 integers, with separate alpha.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional class Ids for the points.
    ///
    /// The [components.ClassId] provides colors if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
    //TODO(ab): add `Label` and  `ShowLabels` components
    //TODO(ab): add `Altitude` component
}
