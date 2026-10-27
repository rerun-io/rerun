// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Flattened latitude/longitude rings for one geospatial polygon.
#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "unstable")]
pub struct GeoPolygonData {
    /// Flattened ring positions in EPSG:4326 latitude/longitude order.
    pub lat_lon: Vec<rerun::encodings::DVec2D>,

    /// Number of positions in each ring, exterior first, followed by holes.
    ///
    /// Their sum must equal the number of positions in `lat_lon`.
    pub ring_lengths: Vec<u64>,
}
