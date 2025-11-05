use std::collections::hash_map::Entry;

use nohash_hasher::IntMap;

use re_types::components::TransformFrameId;
use re_types::{TransformFrameIdHash, archetypes};

/// Frame id registry for resolving frame id hashes back to frame ids.
#[derive(Default)]
pub struct FrameIdRegistry {
    frame_id_lookup_table: IntMap<TransformFrameIdHash, TransformFrameId>,
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
    /// Implementation note:
    /// Having the registration of frame ids separate from other frame id related bookkeeping makes things more modular
    /// at the price of additional overhead. However, we generally assume that retrieving `TransformFrameId`/`TransformFrameIdHash` from a string is fast.
    pub fn register_all_frames_in_chunk(&mut self, chunk: &re_chunk_store::Chunk) {
        // Ensure all implicit frames from this entity all the way up to the root are known.
        // Note that in-between entities may never be mentioned in any chunk, but we want to make sure they're known to the system.
        let mut entity_path = chunk.entity_path();
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

        // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses, we instead use whatever is on `Transform3D`.
        let child_frame_component = archetypes::Transform3D::descriptor_child_frame().component;
        let parent_frame_component = archetypes::Transform3D::descriptor_parent_frame().component;
        for frame_id_strings in chunk
            .iter_slices::<String>(child_frame_component)
            .chain(chunk.iter_slices::<String>(parent_frame_component))
        {
            for frame_id_string in frame_id_strings {
                let frame_id_hash = TransformFrameIdHash::from_str(frame_id_string.as_str());
                self.frame_id_lookup_table
                    .entry(frame_id_hash)
                    .or_insert_with(|| TransformFrameId::new(frame_id_string.as_str()));
            }
        }
    }

    /// Iterates over all known frame ids.
    ///
    /// Mostly useful for testing.
    #[inline]
    pub fn iter_frame_ids(
        &self,
    ) -> impl Iterator<Item = (&TransformFrameIdHash, &TransformFrameId)> {
        self.frame_id_lookup_table.iter()
    }

    /// Hashes of all frame ids ever encountered.
    #[inline]
    pub fn iter_frame_id_hashes(&self) -> impl Iterator<Item = TransformFrameIdHash> {
        self.frame_id_lookup_table.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::frame_id_registry::FrameIdRegistry;
    use re_chunk_store::Chunk;
    use re_log_types::TimePoint;
    use re_types::components::TransformFrameId;
    use re_types::{TransformFrameIdHash, archetypes};

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
