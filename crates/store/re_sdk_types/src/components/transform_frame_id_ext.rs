use re_log_types::EntityPath;

use crate::components::TransformFrameId;

impl TransformFrameId {
    /// The prefix used for implicit transform frames derived from entity paths.
    pub const ENTITY_HIERARCHY_PREFIX: &str = "tf#";

    /// Create a new [`TransformFrameId`] from a string.
    pub fn new(frame_id_name: &str) -> Self {
        frame_id_name.into()
    }

    /// Create a [`TransformFrameId`] from an [`EntityPath`].
    ///
    /// The resulting [`TransformFrameId`] represents the implicit transform frame
    /// at that entity path.
    #[inline]
    pub fn from_entity_path(entity_path: &EntityPath) -> Self {
        format!("{}{}", Self::ENTITY_HIERARCHY_PREFIX, entity_path).into()
    }

    /// Check if this an implicit transform frame derived from an [`EntityPath`].
    #[inline]
    pub fn is_entity_path_derived(&self) -> bool {
        self.0.starts_with(Self::ENTITY_HIERARCHY_PREFIX)
    }

    /// If this is a [`TransformFrameId`] derived from an [`EntityPath`], return that [`EntityPath`].
    #[inline]
    pub fn as_entity_path(&self) -> Option<EntityPath> {
        self.0
            .strip_prefix(Self::ENTITY_HIERARCHY_PREFIX)
            .map(EntityPath::from)
    }
}

impl std::hash::Hash for TransformFrameId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&crate::TransformFrameIdHash::new(self), state);
    }
}

impl std::fmt::Display for TransformFrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::EntityPath;

    use crate::components::TransformFrameId;

    #[test]
    fn test_new_frame_id() {
        let frame_id = TransformFrameId::new("looks_like_a_frame");
        assert_eq!(frame_id.as_entity_path(), None);
        assert!(!frame_id.is_entity_path_derived());
    }

    #[test]
    fn test_entity_path_frame_id_roundtrip() {
        let path = EntityPath::from("/this/is/my/path/there/are/many/like/it/but/this/is/mine");

        let frame_id = TransformFrameId::from_entity_path(&path);
        assert_eq!(frame_id.as_entity_path(), Some(path));
        assert!(frame_id.is_entity_path_derived());
    }

    #[test]
    fn test_no_entity_path_frame_collision() {
        let path = EntityPath::from("looks_like_a_frame");
        let frame_id = TransformFrameId::new("looks_like_a_frame");

        assert_ne!(frame_id, TransformFrameId::from_entity_path(&path));
    }
}
