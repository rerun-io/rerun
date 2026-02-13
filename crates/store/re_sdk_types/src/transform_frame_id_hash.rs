use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, EntityPathHash};

use crate::components::TransformFrameId;

/// Hash for [`TransformFrameId`].
///
/// Just like [`EntityPathHash`] we assume this hash to be collision free.
///
/// Almost always, instead of using [`TransformFrameId`] directly, `re_tf` uses this hash.
/// It is assumed to be collision free.
///
/// For fast handling or Rerun's built-in entity path based transform hierarchy,
/// the [`TransformFrameIdHash`] of an entity path derived frame is guaranteed to be exactly the same as [`EntityPathHash`].
/// Therefore, whenever possible, you should create [`TransformFrameIdHash`] directly from [`EntityPath`],
/// without going via [`TransformFrameId`].
///
/// There's no `Into` conversions for entity paths in order to keep these conversions explicit,
/// marking clearly where we retrieve the implicit frame id's of an entity path.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub struct TransformFrameIdHash(Hash64);

impl re_byte_size::SizeBytes for TransformFrameIdHash {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

impl std::hash::Hash for TransformFrameIdHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl std::cmp::PartialEq for TransformFrameIdHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl From<&TransformFrameId> for TransformFrameIdHash {
    #[inline]
    fn from(value: &TransformFrameId) -> Self {
        Self::new(value)
    }
}

impl nohash_hasher::IsEnabled for TransformFrameIdHash {}

impl std::fmt::Debug for TransformFrameIdHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TransformFrameIdHash({:016X})", self.0.hash64())
    }
}

impl TransformFrameIdHash {
    /// Hash salt used to avoid hash collisions of frame ids & entity paths with the same name.
    const NON_ENTITY_PATH_SALT: u8 = 123;

    /// Create a new [`TransformFrameIdHash`] from a [`TransformFrameId`]
    ///
    /// If your [`TransformFrameId`] is derived from an [`EntityPath`], avoid this method
    /// and use [`Self::from_entity_path`] for faster conversion.
    #[inline]
    pub fn new(value: &TransformFrameId) -> Self {
        if let Some(path) = value.as_entity_path() {
            Self::from_entity_path(&path)
        } else {
            Self(Hash64::hash((value.as_str(), Self::NON_ENTITY_PATH_SALT)))
        }
    }

    /// Create a new [`TransformFrameIdHash`] from a string representing a [`TransformFrameId`].
    #[inline]
    #[expect(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> Self {
        Self::from_str_with_optional_derived_path(value).0
    }

    /// Create a new [`TransformFrameIdHash`] from a string representing a [`TransformFrameId`].
    ///
    /// If the string was an entity path derived frame id, also returns that entity path.
    #[inline]
    pub fn from_str_with_optional_derived_path(value: &str) -> (Self, Option<EntityPath>) {
        if let Some(path) = value.strip_prefix(TransformFrameId::ENTITY_HIERARCHY_PREFIX) {
            let path = EntityPath::parse_forgiving(path);
            (Self::from_entity_path(&path), Some(path))
        } else {
            (
                Self(Hash64::hash((value, Self::NON_ENTITY_PATH_SALT))),
                None,
            )
        }
    }

    /// Fast path for creating a [`TransformFrameIdHash`] directly from an [`EntityPath`].
    ///
    /// The resulting [`TransformFrameIdHash`] represents the implicit transform frame
    /// at that entity path.
    #[inline]
    pub fn from_entity_path(path: &EntityPath) -> Self {
        Self(Hash64::from_u64(path.hash64()))
    }

    /// Fast path for creating a [`TransformFrameIdHash`] directly from an [`EntityPathHash`].
    ///
    /// The resulting [`TransformFrameIdHash`] represents the implicit transform frame
    /// at that entity path.
    #[inline]
    pub fn from_entity_path_hash(path: EntityPathHash) -> Self {
        Self(Hash64::from_u64(path.hash64()))
    }

    /// Convert the [`TransformFrameIdHash`] to an [`EntityPathHash`].
    ///
    /// ⚠️ If this [`TransformFrameIdHash`] is not derived from an [`EntityPath`],
    /// this will return a virtually random [`EntityPathHash`].
    /// There is currently no way to determine whether this is the case.
    #[inline]
    pub fn as_entity_path_hash(&self) -> EntityPathHash {
        EntityPathHash::from_u64(self.0.hash64())
    }

    /// Get the [`TransformFrameIdHash`] of the root of the entity path tree.
    #[inline]
    pub fn entity_path_hierarchy_root() -> Self {
        Self::from_entity_path(&EntityPath::root())
    }

    /// Get the raw hash value of the [`TransformFrameIdHash`].
    #[inline]
    pub fn hash(&self) -> u64 {
        self.0.hash64()
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::EntityPath;

    use crate::components::TransformFrameId;
    use crate::transform_frame_id_hash::TransformFrameIdHash;

    #[test]
    fn test_from_entity_path() {
        let path = EntityPath::from("/this/is/my/path/there/are/many/like/it/but/this/is/mine");
        let derived_frame_id = TransformFrameId::from_entity_path(&path);

        assert_eq!(
            TransformFrameIdHash::new(&derived_frame_id),
            TransformFrameIdHash::from_entity_path(&path)
        );
        assert_eq!(
            TransformFrameIdHash::new(&derived_frame_id),
            TransformFrameIdHash::from_entity_path_hash(path.hash())
        );
        assert_eq!(
            TransformFrameIdHash::from(&derived_frame_id),
            TransformFrameIdHash::from_entity_path_hash(path.hash())
        );
        assert_eq!(
            TransformFrameIdHash::from_str(&format!("tf#{path}")),
            TransformFrameIdHash::from_entity_path_hash(path.hash())
        );
        assert_eq!(
            TransformFrameIdHash::from_str_with_optional_derived_path(&format!("tf#{path}")),
            (
                TransformFrameIdHash::from_entity_path_hash(path.hash()),
                Some(path.clone())
            )
        );

        // Sanity check: parse a string that could be an entity path, but it's not a built-in frame id.
        assert_eq!(
            TransformFrameIdHash::from_str_with_optional_derived_path(&format!("{path}")),
            (
                TransformFrameIdHash::new(&TransformFrameId::new(&format!("{path}"))),
                None
            )
        );
    }

    #[test]
    fn test_no_entity_path_frame_collision() {
        let frame_id = TransformFrameId::new("looks_like_a_frame");
        let path = EntityPath::from("looks_like_a_frame");

        assert_ne!(
            TransformFrameIdHash::new(&frame_id),
            TransformFrameIdHash::from_entity_path(&path)
        );
        assert_ne!(
            TransformFrameIdHash::new(&frame_id),
            TransformFrameIdHash::from_entity_path_hash(path.hash())
        );
        assert_ne!(
            TransformFrameIdHash::from(&frame_id),
            TransformFrameIdHash::from_entity_path(&path)
        );
        assert_ne!(
            TransformFrameIdHash::from_str(frame_id.as_str()),
            TransformFrameIdHash::from_entity_path(&path)
        );
    }
}
