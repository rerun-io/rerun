use super::DisconnectedSpace;

impl From<bool> for DisconnectedSpace {
    fn from(b: bool) -> Self {
        Self(b)
    }
}

impl Default for DisconnectedSpace {
    fn default() -> Self {
        Self(true)
    }
}
