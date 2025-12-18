use std::collections::hash_map::Entry;

use nohash_hasher::{IntMap, IntSet};
use re_byte_size::SizeBytes;
use re_log_types::{EntityPath, EntityPathHash};
use re_sdk_types::components::TransformFrameId;
use re_sdk_types::{TransformFrameIdHash, archetypes};

/// Provides context around frame id hashes.
pub struct FrameIdRegistry {
    /// A lookup table for resolving frame id hashes back to frame ids.
    frame_id_lookup_table: IntMap<TransformFrameIdHash, TransformFrameId>,

    /// Holds all frame ids per entity that were logged as a child frame.
    child_frames_per_entity: IntMap<EntityPathHash, IntSet<TransformFrameIdHash>>,
}

impl Default for FrameIdRegistry {
    fn default() -> Self {
        let root_frame_id_hash = TransformFrameIdHash::entity_path_hierarchy_root();
        Self {
            // Always register the root frame id since empty stores always have the root present in their `EntityTree`!
            // Not registering it from the start would mean that when a new store iterates over all its entities,
            // it would find that the root-derived frame id is missing!
            frame_id_lookup_table: std::iter::once((
                root_frame_id_hash,
                TransformFrameId::from_entity_path(&EntityPath::root()),
            ))
            .collect(),

            // This is initially empty, because implicit frames are not relevant to this map.
            child_frames_per_entity: Default::default(),
        }
    }
}

impl SizeBytes for FrameIdRegistry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            frame_id_lookup_table,
            child_frames_per_entity,
        } = self;

        frame_id_lookup_table.total_size_bytes() + child_frames_per_entity.total_size_bytes()
    }
}

impl FrameIdRegistry {
    /// Looks up a frame ID by its hash.
    ///
    /// Returns `None` if the frame id hash was never encountered.
    #[inline]
    pub fn lookup_frame_id(
        &self,
        frame_id_hash: TransformFrameIdHash,
    ) -> Option<&TransformFrameId> {
        self.frame_id_lookup_table.get(&frame_id_hash)
    }

    /// Registers all frame ids mentioned in a chunk, including frames implied by the chunk's entity and its parents.
    ///
    /// The latter is important since entity-derived ("implicit") frames have an identity connection to their parents.
    /// This means that it's very common to query about all subpaths in a hierarchy even if those subpaths don't hold any data themselves.
    ///
    /// Implementation note:
    /// Having the registration of frame ids separate from other frame id related bookkeeping makes things more modular
    /// at the price of additional overhead. However, we generally assume that retrieving `TransformFrameId`/`TransformFrameIdHash` from a string is fast.
    pub fn register_all_frames_in_chunk(&mut self, chunk: &re_chunk_store::Chunk) {
        self.register_frame_id_from_entity_path(chunk.entity_path());

        let identifier_child_frame = archetypes::Transform3D::descriptor_child_frame().component;

        let frame_components = [
            identifier_child_frame,
            archetypes::Transform3D::descriptor_parent_frame().component,
            archetypes::Pinhole::descriptor_child_frame().component,
            archetypes::Pinhole::descriptor_parent_frame().component,
            archetypes::CoordinateFrame::descriptor_frame().component,
        ];

        for component in frame_components {
            for frame_id_strings in chunk.iter_slices::<String>(component) {
                for frame_id_string in frame_id_strings {
                    let (frame_id_hash, entity_path) =
                        TransformFrameIdHash::from_str_with_optional_derived_path(
                            frame_id_string.as_str(),
                        );

                    // Keep track of child frames and which entities they belong to.
                    if component == identifier_child_frame {
                        self.child_frames_per_entity
                            .entry(chunk.entity_path().hash())
                            .or_default()
                            .insert(frame_id_hash);
                    }

                    if let Entry::Vacant(e) = self.frame_id_lookup_table.entry(frame_id_hash) {
                        e.insert(TransformFrameId::new(frame_id_string.as_str()));

                        // If this was an entity path, we didn't know about this one before.
                        // Make sure we also add all parents!
                        if let Some(entity_path) = entity_path.and_then(|path| path.parent()) {
                            self.register_frame_id_from_entity_path(&entity_path);
                        }
                    }
                }
            }
        }
    }

    /// Registers entity path derived frame id and all its parents.
    fn register_frame_id_from_entity_path(&mut self, entity_path: &EntityPath) {
        // Ensure all implicit frames from this entity all the way up to the root are known.
        // Note that in-between entities may never be mentioned in any chunk, but we want to make sure they're known to the system.
        let mut entity_path = entity_path; // Have to redeclare to make borrow-checker happy.
        let mut parent;
        loop {
            // Note that we try to avoid computing `TransformFrameId` as much as we can since it has to string-concat,
            // so compared to `TransformFrameIdHash,` is it _relatively_ expensive to compute.
            match self
                .frame_id_lookup_table
                .entry(TransformFrameIdHash::from_entity_path(entity_path))
            {
                Entry::Occupied(_) => {
                    break;
                }
                Entry::Vacant(e) => e.insert(TransformFrameId::from_entity_path(entity_path)),
            };

            parent = entity_path.parent();
            if let Some(parent) = parent.as_ref() {
                entity_path = parent;
            } else {
                break;
            }
        }
    }

    /// Iterates over all known frame ids.
    #[inline]
    pub fn iter_frame_ids(
        &self,
    ) -> impl Iterator<Item = (&TransformFrameIdHash, &TransformFrameId)> {
        self.frame_id_lookup_table.iter()
    }

    /// Iterates over all known entities with child frame components.
    pub fn iter_entities_with_child_frames(
        &self,
    ) -> impl Iterator<Item = (&EntityPathHash, &IntSet<TransformFrameIdHash>)> {
        self.child_frames_per_entity.iter()
    }

    /// Hashes of all frame ids ever encountered.
    #[inline]
    pub fn iter_frame_id_hashes(&self) -> impl Iterator<Item = TransformFrameIdHash> {
        self.frame_id_lookup_table.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use re_chunk_store::Chunk;
    use re_log_types::{EntityPath, TimePoint};
    use re_sdk_types::components::TransformFrameId;
    use re_sdk_types::{TransformFrameIdHash, archetypes};

    use crate::frame_id_registry::FrameIdRegistry;

    #[test]
    fn test_entity_path_derived_frame_in_data() {
        let mut registry = FrameIdRegistry::default();

        registry.register_all_frames_in_chunk(
            &Chunk::builder("dontcare")
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::default().with_child_frame("tf#/some/deep/path"),
                )
                .build()
                .unwrap(),
        );

        let mut candidate = Some(EntityPath::from("some/deep/path"));
        while let Some(path) = candidate {
            assert_eq!(
                registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(&path)),
                Some(&TransformFrameId::from_entity_path(&path))
            );
            candidate = path.parent();
        }
    }

    #[test]
    fn test_register_all_ids_in_chunk() {
        let mut registry = FrameIdRegistry::default();

        registry.register_all_frames_in_chunk(
            &Chunk::builder("root/robot/hand/pinky")
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::default().with_many_child_frame(["child0", "child1"]),
                )
                .build()
                .unwrap(),
        );
        registry.register_all_frames_in_chunk(
            &Chunk::builder("root/surprise")
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::default().with_many_parent_frame(["parent0"]),
                )
                .build()
                .unwrap(),
        );
        registry.register_all_frames_in_chunk(
            &Chunk::builder("root/surprise")
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::CoordinateFrame::new("frame0"),
                )
                .build()
                .unwrap(),
        );

        // Verify explicit frame IDs from the first chunk.
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_str("child0")),
            Some(&TransformFrameId::new("child0"))
        );
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_str("child1")),
            Some(&TransformFrameId::new("child1"))
        );

        // Verify explicit frame IDs from the second chunk.
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_str("parent0")),
            Some(&TransformFrameId::new("parent0"))
        );

        // Verify explicit frame IDs from the third chunk.
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_str("frame0")),
            Some(&TransformFrameId::new("frame0"))
        );

        // Verify implicit frame IDs from entity paths.
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(
                &"root/robot/hand/pinky".into()
            )),
            Some(&TransformFrameId::from_entity_path(
                &"root/robot/hand/pinky".into()
            ))
        );
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(
                &"root/robot/hand".into()
            )),
            Some(&TransformFrameId::from_entity_path(
                &"root/robot/hand".into()
            ))
        );
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(&"root/robot".into())),
            Some(&TransformFrameId::from_entity_path(&"root/robot".into()))
        );
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(&"root".into())),
            Some(&TransformFrameId::from_entity_path(&"root".into()))
        );
        assert_eq!(
            registry.lookup_frame_id(TransformFrameIdHash::from_entity_path(
                &"root/surprise".into()
            )),
            Some(&TransformFrameId::from_entity_path(&"root/surprise".into()))
        );
    }
}
