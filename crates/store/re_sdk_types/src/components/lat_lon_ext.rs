use super::LatLon;
use crate::datatypes::DVec2D;

// ---

impl LatLon {
    /// Create a new position.
    #[inline]
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self(DVec2D::new(lat, lon))
    }

    /// The latitude.
    #[inline]
    pub fn latitude(&self) -> f64 {
        self.0.x()
    }

    /// The longitude.
    #[inline]
    pub fn longitude(&self) -> f64 {
        self.0.y()
    }
}
