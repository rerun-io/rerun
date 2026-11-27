use super::GeoPolygon;
use crate::encodings::DVec2D;

impl GeoPolygon {
    /// Construct a polygon from an exterior ring followed by any holes.
    ///
    /// Coordinates are latitude/longitude in North/East-positive degrees.
    pub fn from_lat_lon(
        rings: impl IntoIterator<Item = impl IntoIterator<Item = impl Into<DVec2D>>>,
    ) -> Self {
        let mut lat_lon = Vec::new();
        let mut ring_lengths = Vec::new();
        for ring in rings {
            let start = lat_lon.len();
            lat_lon.extend(ring.into_iter().map(Into::into));
            ring_lengths.push((lat_lon.len() - start) as u64);
        }
        Self(crate::encodings::GeoPolygonData {
            lat_lon,
            ring_lengths: ring_lengths.into(),
        })
    }
}
