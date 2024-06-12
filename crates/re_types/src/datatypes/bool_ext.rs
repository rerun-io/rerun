use std::ops::Deref;

use super::Bool;

impl Deref for Bool {
    type Target = bool;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
