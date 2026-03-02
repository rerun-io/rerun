#![expect(non_snake_case)]

use super::ViewCoordinates2D;
use crate::components;

impl ViewCoordinates2D {
    /// X=Right, Y=Down (default, image/screen convention).
    pub fn RD() -> Self {
        Self::new(components::ViewCoordinates2D::RD)
    }

    /// X=Right, Y=Up (math/plot convention).
    pub fn RU() -> Self {
        Self::new(components::ViewCoordinates2D::RU)
    }

    /// X=Left, Y=Down (horizontally mirrored image).
    pub fn LD() -> Self {
        Self::new(components::ViewCoordinates2D::LD)
    }

    /// X=Left, Y=Up (both axes flipped).
    pub fn LU() -> Self {
        Self::new(components::ViewCoordinates2D::LU)
    }
}
