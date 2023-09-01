use super::TensorId;

impl nohash_hasher::IsEnabled for TensorId {}

// required for [`nohash_hasher`].
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for TensorId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(bytemuck::cast::<[u8; 16], [u64; 2]>(self.uuid)[0]);
    }
}

impl TensorId {
    #[inline]
    pub fn random() -> Self {
        Self {
            uuid: *uuid::Uuid::new_v4().as_bytes(),
        }
    }
}

impl From<uuid::Uuid> for TensorId {
    fn from(value: uuid::Uuid) -> Self {
        Self {
            uuid: *value.as_bytes(),
        }
    }
}
