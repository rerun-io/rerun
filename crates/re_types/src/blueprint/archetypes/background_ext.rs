use crate::{blueprint::components::BackgroundKind, components::Color};

use super::Background;

impl Background {
    pub const DEFAULT_COLOR: Color = Color::WHITE;
}

impl Default for Background {
    #[inline]
    fn default() -> Self {
        Self {
            kind: BackgroundKind::default(),
            color: Some(Background::DEFAULT_COLOR),
        }
    }
}
