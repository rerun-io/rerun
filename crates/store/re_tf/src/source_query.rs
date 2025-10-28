use crate::transform_aspect::TransformAspect;

use nohash_hasher::IntSet;
use re_chunk_store::RangeQuery;
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimelineName};
use re_types::{TransformFrameIdHash, archetypes};

/// Returns all sources ever mentioned in the given time range plus values ahead and before min/max.
///
/// If extended bounds yield no value for times <=min, the entity-derived default frame will automatically be appended
/// since logically it is present whenever there's no source logged.
/// Therefore, the returned list is never empty.
///
/// Note that to build our look-up data-structure in [`crate::TransformResolutionCache`] we have to know the source frame for this entity.
/// Doing so just by looking at this chunk is unfortunately not possible - there may be temporally overlapping chunks,
/// clears and recursive clears, all which have to be taken into account.
/// Therefore, we have to do a range query for the affected range.
pub fn query_sources_in_extended_bounds(
    entity_db: &EntityDb,
    entity_path: &EntityPath,
    timeline: TimelineName,
    aspects: TransformAspect,
    min_time: TimeInt,
    max_time: TimeInt,
) -> IntSet<TransformFrameIdHash> {
    let fallback_source = TransformFrameIdHash::from_entity_path(entity_path);

    if aspects.contains(TransformAspect::Frame) {
        let source_frame_component = archetypes::Transform3D::descriptor_source_frames().component;
        let result = entity_db.storage_engine().cache().range(
            &RangeQuery::new(timeline, AbsoluteTimeRange::new(min_time, max_time))
                .include_extended_bounds(true), // Need to add extended bounds so we know the last known value.
            entity_path,
            [source_frame_component],
        );

        if let Some(chunks) = result.get(source_frame_component) {
            let mut need_to_add_fallback = chunks
                .iter()
                .flat_map(|chunk| chunk.iter_component_indices(timeline, source_frame_component))
                .next()
                .is_none_or(|(first_time, _)| first_time > min_time);

            let mut query_sources_in_extended_bounds: IntSet<TransformFrameIdHash> = chunks
                .iter()
                .flat_map(|chunk| chunk.iter_slices::<String>(source_frame_component))
                .flat_map(|sources| {
                    if sources.is_empty() {
                        // If empty shows up we have to assume the fallback is active.
                        need_to_add_fallback = true;
                    }
                    sources
                        .into_iter()
                        .map(|source| TransformFrameIdHash::from_str(source.as_str()))
                })
                .collect();

            if need_to_add_fallback {
                query_sources_in_extended_bounds.insert(fallback_source);
            }
            return query_sources_in_extended_bounds;
        }
    }

    // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses.
    IntSet::from_iter([fallback_source])
}

#[cfg(test)]
mod tests {
    use super::query_sources_in_extended_bounds;
    use crate::transform_aspect::TransformAspect;

    use nohash_hasher::IntSet;
    use re_chunk_store::Chunk;
    use re_entity_db::EntityDb;
    use re_log_types::{EntityPath, StoreId, TimeInt, TimePoint, TimeType, Timeline};
    use re_types::{TransformFrameIdHash, archetypes};
    use std::sync::Arc;

    #[test]
    fn test_query_sources_per_time_range() -> Result<(), Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(StoreId::random(
            re_log_types::StoreKind::Recording,
            "test_app",
        ));

        let timeline = Timeline::new("time", TimeType::Sequence);

        let entity_path_static = EntityPath::from("static_entry");
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(entity_path_static.clone())
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::update_fields().with_source_frames(["frame0"]),
                )
                .build()?,
        ))?;
        let entity_path_dynamic = EntityPath::from("dynamic_entry");
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(entity_path_dynamic.clone())
                .with_archetype_auto_row(
                    [(timeline, 10)],
                    &archetypes::Transform3D::update_fields().with_source_frames(["frame1"]),
                )
                .with_archetype_auto_row(
                    [(timeline, 20)],
                    &archetypes::Transform3D::update_fields()
                        .with_source_frames(["frame2", "frame3"]),
                )
                .with_archetype_auto_row(
                    [(timeline, 30)],
                    &archetypes::Transform3D::update_fields().with_source_frames(["frame4"]),
                )
                .build()?,
        ))?;

        // Test on static component.
        assert_eq!(
            query_sources_in_extended_bounds(
                &entity_db,
                &entity_path_static,
                *timeline.name(),
                TransformAspect::Frame,
                TimeInt::STATIC,
                100.try_into()?,
            ),
            IntSet::from_iter([TransformFrameIdHash::from_str("frame0")])
        );
        assert_eq!(
            query_sources_in_extended_bounds(
                &entity_db,
                &entity_path_static,
                *timeline.name(),
                TransformAspect::Frame,
                10.try_into()?,
                100.try_into()?,
            ),
            IntSet::from_iter([TransformFrameIdHash::from_str("frame0")])
        );

        // Test on changing component.
        assert_eq!(
            query_sources_in_extended_bounds(
                &entity_db,
                &entity_path_dynamic,
                *timeline.name(),
                TransformAspect::Frame,
                0.try_into()?,
                10000.try_into()?,
            ),
            IntSet::from_iter([
                TransformFrameIdHash::from_entity_path(&entity_path_dynamic),
                TransformFrameIdHash::from_str("frame1"),
                TransformFrameIdHash::from_str("frame2"),
                TransformFrameIdHash::from_str("frame3"),
                TransformFrameIdHash::from_str("frame4")
            ])
        );
        assert_eq!(
            query_sources_in_extended_bounds(
                &entity_db,
                &entity_path_dynamic,
                *timeline.name(),
                TransformAspect::Frame,
                11.try_into()?,
                29.try_into()?,
            ),
            IntSet::from_iter([
                TransformFrameIdHash::from_str("frame1"),
                TransformFrameIdHash::from_str("frame2"),
                TransformFrameIdHash::from_str("frame3"),
                TransformFrameIdHash::from_str("frame4") // Because of extended range
            ])
        );

        // Test for correct behavior if there's no data at all.
        let entity_path = EntityPath::from("nope");
        assert_eq!(
            query_sources_in_extended_bounds(
                &entity_db,
                &entity_path,
                *timeline.name(),
                TransformAspect::Frame,
                42.try_into()?,
                100.try_into()?,
            ),
            IntSet::from_iter([TransformFrameIdHash::from_entity_path(&entity_path)])
        );

        Ok(())
    }
}
