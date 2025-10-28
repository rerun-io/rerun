use crate::transform_aspect::TransformAspect;

use re_chunk_store::RangeQuery;
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimelineName};
use re_types::components::TransformFrameId;
use re_types::{TransformFrameIdHash, archetypes};
use std::collections::BTreeMap;

/// Returns for which source frames the transforms apply within the given time range.
///
/// `min_time`/`max_time` are inclusive.
///
/// Returned map is keyed over the minimum of a range and will always at least contain `min_time`.
/// If there was no value at `min_time` logged directly, the value returned for it is the last known value on this timeline.
///
/// The minimum time will never be lower than `min_time`.
/// The maximum time will never be larger than `max_time`.
pub fn query_sources_per_time_range(
    entity_db: &EntityDb,
    entity_path: &EntityPath,
    timeline: TimelineName,
    aspects: TransformAspect,
    min_time: TimeInt,
    max_time: TimeInt,
) -> BTreeMap<TimeInt, Vec<TransformFrameIdHash>> {
    let fallback_source = TransformFrameIdHash::from_entity_path(entity_path);

    if aspects.contains(TransformAspect::Frame) {
        // To build our look-up data-structure we have to know the source frame for this entity.
        // Doing so just by looking at this chunk is unfortunately not possible - there may be temporally overlapping chunks,
        // clears and recursive clears, all which have to be taken into account.
        // Therefore, we have to do a range query for the affected range.
        let source_frame_component = archetypes::Transform3D::descriptor_source_frames().component;
        let result = entity_db.storage_engine().cache().range(
            &RangeQuery::new(timeline, AbsoluteTimeRange::new(min_time, max_time))
                .include_extended_bounds(true), // Need to add extended bounds so we know the last known value.
            entity_path,
            [source_frame_component],
        );

        if let Some(chunks) = result.get(source_frame_component) {
            let mut query_sources_per_time_range: BTreeMap<TimeInt, Vec<TransformFrameIdHash>> =
                chunks
                    .iter()
                    .flat_map(move |chunk| {
                        itertools::izip!(
                            chunk.iter_component_indices(timeline, source_frame_component),
                            chunk.iter_slices::<String>(source_frame_component),
                        )
                    })
                    .map(|((time, _row), sources)| {
                        if sources.is_empty() {
                            (time, vec![fallback_source])
                        } else {
                            (
                                time,
                                sources
                                    .into_iter()
                                    .map(|source_frame| {
                                        TransformFrameIdHash::new(&TransformFrameId::from(
                                            source_frame,
                                        ))
                                    })
                                    .collect(),
                            )
                        }
                    })
                    .collect();

            if let Some(first_time) = query_sources_per_time_range.keys().next().copied() {
                if first_time < min_time {
                    // Clamp the first value's time to the min range.
                    let first_value = query_sources_per_time_range.remove(&first_time).unwrap();
                    let previous_min_value =
                        query_sources_per_time_range.insert(min_time, first_value);
                    debug_assert!(
                        previous_min_value.is_none(),
                        "Expect extended range query to not extend if range-min lands directly on a present time value."
                    );
                } else if first_time > min_time {
                    // If the first value's time is higher than the min range, we need to add a the fallback entry for everything before.
                    query_sources_per_time_range.insert(min_time, vec![fallback_source]);
                }

                // Extended range may give us a single value larger than the max range. Remove it.
                if let Some(entry) = query_sources_per_time_range.last_entry()
                    && entry.key() > &max_time
                {
                    entry.remove();
                }

                query_sources_per_time_range
            } else {
                BTreeMap::from_iter([(min_time, vec![fallback_source])])
            }
        } else {
            BTreeMap::from_iter([(min_time, vec![fallback_source])])
        }
    } else {
        // TODO(RR-2627, RR-2680): Custom source is not supported yet for Pinhole & Poses.
        BTreeMap::from_iter([(min_time, vec![fallback_source])])
    }
}

#[cfg(test)]
mod tests {
    use super::query_sources_per_time_range;
    use crate::transform_aspect::TransformAspect;

    use re_chunk_store::Chunk;
    use re_entity_db::EntityDb;
    use re_log_types::{EntityPath, StoreId, TimeInt, TimePoint, TimeType, Timeline};
    use re_types::{TransformFrameIdHash, archetypes};
    use std::collections::BTreeMap;
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
            query_sources_per_time_range(
                &entity_db,
                &entity_path_static,
                *timeline.name(),
                TransformAspect::Frame,
                TimeInt::STATIC,
                100.try_into()?,
            ),
            BTreeMap::from_iter([(
                TimeInt::STATIC,
                vec![TransformFrameIdHash::from_str("frame0")]
            )])
        );
        assert_eq!(
            query_sources_per_time_range(
                &entity_db,
                &entity_path_static,
                *timeline.name(),
                TransformAspect::Frame,
                10.try_into()?,
                100.try_into()?,
            ),
            BTreeMap::from_iter([(
                10.try_into()?,
                vec![TransformFrameIdHash::from_str("frame0")]
            )])
        );

        // Test on changing component.
        assert_eq!(
            query_sources_per_time_range(
                &entity_db,
                &entity_path_dynamic,
                *timeline.name(),
                TransformAspect::Frame,
                0.try_into()?,
                10000.try_into()?,
            ),
            BTreeMap::from_iter([
                (
                    0.try_into()?,
                    vec![TransformFrameIdHash::from_entity_path(&entity_path_dynamic)]
                ),
                (
                    10.try_into()?,
                    vec![TransformFrameIdHash::from_str("frame1")]
                ),
                (
                    20.try_into()?,
                    vec![
                        TransformFrameIdHash::from_str("frame2"),
                        TransformFrameIdHash::from_str("frame3")
                    ]
                ),
                (
                    30.try_into()?,
                    vec![TransformFrameIdHash::from_str("frame4")]
                )
            ])
        );
        assert_eq!(
            query_sources_per_time_range(
                &entity_db,
                &entity_path_dynamic,
                *timeline.name(),
                TransformAspect::Frame,
                11.try_into()?,
                29.try_into()?,
            ),
            BTreeMap::from_iter([
                (
                    11.try_into()?,
                    vec![TransformFrameIdHash::from_str("frame1")]
                ),
                (
                    20.try_into()?,
                    vec![
                        TransformFrameIdHash::from_str("frame2"),
                        TransformFrameIdHash::from_str("frame3")
                    ]
                ),
            ])
        );

        // Test for correct behavior if there's no data at all.
        let entity_path = EntityPath::from("nope");
        assert_eq!(
            query_sources_per_time_range(
                &entity_db,
                &entity_path,
                *timeline.name(),
                TransformAspect::Frame,
                42.try_into()?,
                100.try_into()?,
            ),
            BTreeMap::from_iter([(
                42.try_into()?,
                vec![TransformFrameIdHash::from_entity_path(&entity_path)]
            )])
        );

        Ok(())
    }
}
