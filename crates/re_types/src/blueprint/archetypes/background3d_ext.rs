use crate::{blueprint::components::Background3DKind, components::Color};

use super::Background3D;

impl Background3D {
    pub const DEFAULT_COLOR: Color = Color::WHITE;
}

impl Default for Background3D {
    fn default() -> Self {
        Self {
            kind: Background3DKind::DirectionalGradientDark,
            color: Some(Background3D::DEFAULT_COLOR),
        }
    }
}
