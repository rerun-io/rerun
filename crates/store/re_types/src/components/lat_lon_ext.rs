use crate::datatypes::DVec2D;

use super::LatLon;

// ---

impl LatLon {
    /// The origin.
    pub const ZERO: Self = Self::new(0.0, 0.0);

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

#[cfg(feature = "mint")]
impl From<LatLon> for mint::Point2<f64> {
    #[inline]
    fn from(position: LatLon) -> Self {
        Self {
            x: position.x(),
            y: position.y(),
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Point2<f64>> for LatLon {
    #[inline]
    fn from(position: mint::Point2<f64>) -> Self {
        Self(DVec2D([position.x, position.y]))
    }
}
