// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A geospatial line string expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[python(aliases = "datatypes.DVec2DArrayLike | npt.NDArray[np.float64]")]
#[python(array_aliases = "npt.NDArray[np.float64]")]
#[rust(derive(Default, PartialEq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct GeoLineString {
    pub lat_lon: Vec<rerun::datatypes::DVec2D>,
}
