// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A geospatial polygon with an exterior ring and optional interior rings (holes).
///
/// Positions use [EPSG:4326](https://epsg.io/4326) latitude and longitude in North/East-positive degrees.
/// Rings may be open or closed and may have either winding order.
/// The first ring is the exterior; subsequent rings are holes.
///
/// Rings must describe a valid polygon with non-intersecting boundaries and holes inside the exterior.
/// Split rings crossing the antimeridian into separate polygons before logging.
/// The map clamps polar positions to the Web Mercator latitude limit.
#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "unstable")]
pub struct GeoPolygon {
    pub polygon: rerun::encodings::GeoPolygonData,
}
