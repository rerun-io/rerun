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
/// This is used to build our look-up data-structure in [`crate::TransformResolutionCache`].
/// There, we have to know the source frame for a given entity.
/// Doing so just by looking at individual chunks as they come in is unfortunately not possible:
/// there may be temporally overlapping chunks, clears and recursive clears, all which have to be taken into account.
/// Therefore, we have to do a range query for the affected range as done here.
///
/// Since we don't allow source frames to be mentioned in multiple entities over time,
/// this is safe to use for a conservative estimate of when a source frame may have changed.
/// If we did allow for it, this kind of conservative set would cause issues!
/// Example:
/// Let's take this setup violating said invariant:
/// ```raw
/// /entity0/ time=0: sources: A, B
///           time=1: sources: C
/// /entity1/ time=1: sources: A, B
/// ```
/// `query_sources_in_extended_bounds` would now report A, B and C.
/// Leaving us to incorrectly believe that for `/entity0/ @ time=1` there might be data for A & B when we
/// actually need to lookup `/entity1/`.
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
        let source_frame_component = archetypes::Transform3D::descriptor_source_frame().component;
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

/// Extracts all the source frames mentioned in a static chunk.
pub fn query_source_frames_in_static_chunk(
    chunk: &re_chunk_store::Chunk,
    aspects: TransformAspect,
) -> IntSet<TransformFrameIdHash> {
    debug_assert!(chunk.is_static());

    // We only care about static time, so unlike on temporal chunks, we can just check the source frame list directly from the chunk if any.
    if aspects.contains(TransformAspect::Frame) {
        let source_frame_component = archetypes::Transform3D::descriptor_source_frame().component;
        if let Some(sources) = chunk.iter_slices::<String>(source_frame_component).next() {
            let sources: IntSet<_> = sources
                .into_iter()
                .map(|source| TransformFrameIdHash::from_str(source.as_str()))
                .collect();
            if !sources.is_empty() {
                return sources;
            }
        }
    }

    // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses.
    IntSet::from_iter([TransformFrameIdHash::from_entity_path(chunk.entity_path())])
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
                    &archetypes::Transform3D::update_fields().with_source_frame("frame0"),
                )
                .build()?,
        ))?;
        let entity_path_dynamic = EntityPath::from("dynamic_entry");
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(entity_path_dynamic.clone())
                .with_archetype_auto_row(
                    [(timeline, 10)],
                    &archetypes::Transform3D::update_fields().with_source_frame("frame1"),
                )
                .with_archetype_auto_row(
                    [(timeline, 20)],
                    // TODO(RR-2799): Allow multiple sources for a single entit. Using `with_many_source_frame` is a bit of a hack to get there.
                    &archetypes::Transform3D::update_fields()
                        .with_many_source_frame(["frame2", "frame3"]),
                )
                .with_archetype_auto_row(
                    [(timeline, 30)],
                    &archetypes::Transform3D::update_fields().with_source_frame("frame4"),
                )
                .build()?,
        ))?;

        // Test on chunk with static data.
        // Note that this is *not* a made-up or redundant case that should be covered by `query_sources_static_chunk`.
        // Rather, this can be of interest when there are other temporal data on the same entity, but sources happen to be static.
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

    #[test]
    fn test_query_source_frames_in_static_chunk() -> Result<(), Box<dyn std::error::Error>> {
        let entity_path = EntityPath::from("test_entry");

        // Test with an empty chunk.
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype_auto_row(TimePoint::STATIC, &archetypes::Transform3D::update_fields())
            .build()?;
        assert_eq!(
            super::query_source_frames_in_static_chunk(&chunk, TransformAspect::Frame),
            IntSet::from_iter([TransformFrameIdHash::from_entity_path(&entity_path)])
        );

        // Test chunk with multiple sources
        let chunk = Chunk::builder(entity_path.clone())
            .with_archetype_auto_row(
                TimePoint::STATIC,
                // TODO(RR-2799): Allow multiple sources for a single entit. Using `with_many_source_frame` is a bit of a hack to get there.
                &archetypes::Transform3D::update_fields()
                    .with_many_source_frame(["frame1", "frame2"]),
            )
            .build()?;

        assert_eq!(
            super::query_source_frames_in_static_chunk(&chunk, TransformAspect::Frame),
            IntSet::from_iter([
                TransformFrameIdHash::from_str("frame1"),
                TransformFrameIdHash::from_str("frame2")
            ])
        );

        Ok(())
    }
}
