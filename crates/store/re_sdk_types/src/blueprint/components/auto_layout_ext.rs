use super::AutoLayout;
use crate::encodings::Bool;

impl Default for AutoLayout {
    #[inline]
    fn default() -> Self {
        Self(Bool(true))
    }
}
