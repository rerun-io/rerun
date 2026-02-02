use super::{TextLogColumn, TextLogColumnKind};

impl Default for TextLogColumn {
    #[inline]
    fn default() -> Self {
        Self {
            kind: TextLogColumnKind::default(),
            visible: true.into(),
        }
    }
}
