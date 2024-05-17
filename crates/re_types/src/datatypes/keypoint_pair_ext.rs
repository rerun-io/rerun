use super::{KeypointId, KeypointPair};

impl From<(u16, u16)> for KeypointPair {
    fn from(value: (u16, u16)) -> Self {
        Self {
            keypoint0: value.0.into(),
            keypoint1: value.1.into(),
        }
    }
}

impl From<(KeypointId, KeypointId)> for KeypointPair {
    fn from(value: (KeypointId, KeypointId)) -> Self {
        Self {
            keypoint0: value.0,
            keypoint1: value.1,
        }
    }
}

impl KeypointPair {
    /// Create a vector of [`KeypointPair`] from an array of tuples.
    pub fn vec_from<T: Into<Self>, const N: usize>(value: [T; N]) -> Vec<Self> {
        value.into_iter().map(|v| v.into()).collect()
    }
}
