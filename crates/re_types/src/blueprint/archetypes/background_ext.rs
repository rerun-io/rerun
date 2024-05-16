use crate::{blueprint::components::BackgroundKind, components::Color};

use super::Background;

impl Background {
    /// Default background of 2D space views.
    pub const DEFAULT_2D: Self = Self {
        kind: BackgroundKind::SolidColor,
        color: Some(Self::DEFAULT_COLOR_2D),
    };

    /// Default background of 3D space views.
    pub const DEFAULT_3D: Self = Self {
        kind: BackgroundKind::GradientDark,
        color: Some(Self::DEFAULT_COLOR_3D),
    };

    /// Default background color of 2D space views.
    pub const DEFAULT_COLOR_2D: Color = Color::BLACK;

    /// Default background color of 3D space views.
    pub const DEFAULT_COLOR_3D: Color = Color::WHITE;
}
