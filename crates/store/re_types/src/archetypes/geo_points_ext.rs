use super::GeoPoints;

impl GeoPoints {
    /// Create a new `GeoPoints` from [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
    #[inline]
    pub fn from_lat_lon(
        positions: impl IntoIterator<Item = impl Into<crate::components::LatLon>>,
    ) -> Self {
        Self::new(positions)
    }
}
