use super::DisconnectedSpace;

impl Default for DisconnectedSpace {
    #[inline]
    fn default() -> Self {
        Self(true.into())
    }
}
