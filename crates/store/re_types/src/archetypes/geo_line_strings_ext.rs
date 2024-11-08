use super::GeoLineStrings;

impl GeoLineStrings {
    /// Create a new `GeoLineStrings` from [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
    #[inline]
    pub fn from_lat_lon(
        line_strings: impl IntoIterator<Item = impl Into<crate::components::GeoLineString>>,
    ) -> Self {
        Self::new(line_strings)
    }
}
