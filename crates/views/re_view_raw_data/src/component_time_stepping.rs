use re_chunk::{ComponentIdentifier, EarliestAtQuery, LatestAtQuery, TimelineName};
use re_chunk_store::{ChunkStore, ChunkTrackingMode};
use re_log_types::{EntityPath, TimeInt};

/// Returns the next loaded event for the selected components, strictly after `after`.
///
/// Static values override temporal values and do not contribute navigation events.
pub fn next_time_for_components(
    store: &ChunkStore,
    timeline: &TimelineName,
    entity_path: &EntityPath,
    components: &[ComponentIdentifier],
    after: TimeInt,
) -> Option<TimeInt> {
    re_tracing::profile_function!();

    let query = EarliestAtQuery::new(
        *timeline,
        TimeInt::new_temporal(after.as_i64().checked_add(1)?),
    );
    components
        .iter()
        .filter_map(|&component| {
            store
                .earliest_at_relevant_chunks(
                    // This runs every frame to update navigation buttons. Keep it passive:
                    // even ReportTransient would request downloads and protect chunks from GC.
                    ChunkTrackingMode::Ignore,
                    &query,
                    entity_path,
                    component,
                )
                .chunks
                .iter()
                .filter(|chunk| !chunk.is_static())
                .filter_map(|chunk| chunk.earliest_at(&query, component)?.index(Some(timeline)))
                .map(|(time, _)| time)
                .min()
        })
        .min()
}

/// Returns the previous loaded event for the selected components, strictly before `before`.
///
/// Static values override temporal values and do not contribute navigation events.
pub fn prev_time_for_components(
    store: &ChunkStore,
    timeline: &TimelineName,
    entity_path: &EntityPath,
    components: &[ComponentIdentifier],
    before: TimeInt,
) -> Option<TimeInt> {
    re_tracing::profile_function!();

    if before <= TimeInt::MIN {
        return None;
    }
    latest_time_for_components(
        store,
        timeline,
        entity_path,
        components,
        TimeInt::new_temporal(before.as_i64() - 1),
    )
}

/// Returns the latest loaded event for the selected components at or before `at`.
///
/// Static values override temporal values and do not contribute navigation events.
pub fn latest_time_for_components(
    store: &ChunkStore,
    timeline: &TimelineName,
    entity_path: &EntityPath,
    components: &[ComponentIdentifier],
    at: TimeInt,
) -> Option<TimeInt> {
    re_tracing::profile_function!();

    let query = LatestAtQuery::new(*timeline, at);
    components
        .iter()
        .filter_map(|&component| {
            store
                .latest_at_relevant_chunks(
                    // This runs every frame to update navigation buttons. Keep it passive:
                    // even ReportTransient would request downloads and protect chunks from GC.
                    ChunkTrackingMode::Ignore,
                    &query,
                    entity_path,
                    component,
                )
                .chunks
                .iter()
                .filter(|chunk| !chunk.is_static())
                .filter_map(|chunk| chunk.latest_at(&query, component)?.index(Some(timeline)))
                .map(|(time, _)| time)
                .max()
        })
        .max()
}

#[cfg(test)]
mod tests {
    use re_chunk::Chunk;
    use re_chunk_store::ChunkStoreConfig;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{StoreId, StoreKind, TimePoint, Timeline};

    use super::*;

    fn timeline() -> Timeline {
        Timeline::new_sequence("frame")
    }

    fn points() -> ComponentIdentifier {
        MyPoints::descriptor_points().component
    }

    fn colors() -> ComponentIdentifier {
        MyPoints::descriptor_colors().component
    }

    fn t(time: i64) -> TimeInt {
        TimeInt::new_temporal(time)
    }

    fn next(
        store: &ChunkStore,
        components: &[ComponentIdentifier],
        after: TimeInt,
    ) -> Option<TimeInt> {
        next_time_for_components(
            store,
            timeline().name(),
            &"points".into(),
            components,
            after,
        )
    }

    fn prev(
        store: &ChunkStore,
        components: &[ComponentIdentifier],
        before: TimeInt,
    ) -> Option<TimeInt> {
        prev_time_for_components(
            store,
            timeline().name(),
            &"points".into(),
            components,
            before,
        )
    }

    fn insert(store: &mut ChunkStore, chunk: Chunk) {
        store.insert_chunk(&std::sync::Arc::new(chunk)).unwrap();
    }

    /// Points at even frames and colors at odd frames, so a subset never coincides with the whole.
    fn test_store() -> ChunkStore {
        let mut store = ChunkStore::new(
            StoreId::random(StoreKind::Recording, "test_app"),
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        let mut builder = Chunk::builder("points");
        for frame in [0, 3, 2, 1, 4] {
            builder = builder.with_archetype_auto_row(
                TimePoint::from([(timeline(), frame)]),
                &if frame % 2 == 0 {
                    MyPoints::new([MyPoint::new(frame as f32, 0.0)])
                } else {
                    MyPoints::update_fields().with_colors([MyColor::from_rgb(255, 0, 0)])
                },
            );
        }
        insert(&mut store, builder.build().unwrap());

        store
    }

    #[test]
    fn latest_event_includes_the_query_time() {
        let store = test_store();

        assert_eq!(
            latest_time_for_components(
                &store,
                timeline().name(),
                &"points".into(),
                &[points()],
                t(2),
            ),
            Some(t(2)),
        );
    }

    #[test]
    fn next_time_skips_components_not_asked_for() {
        let store = test_store();

        assert_eq!(next(&store, &[points(), colors()], t(0)), Some(t(1)));
        assert_eq!(next(&store, &[points()], t(0)), Some(t(2)));
        assert_eq!(next(&store, &[colors()], t(0)), Some(t(1)));
        assert_eq!(next(&store, &[colors()], t(1)), Some(t(3)));
        assert_eq!(next(&store, &[colors()], t(3)), None);
        assert_eq!(next(&store, &[points()], t(4)), None);
        assert_eq!(next(&store, &[], t(0)), None);
    }

    #[test]
    fn prev_time_skips_components_not_asked_for() {
        let store = test_store();

        assert_eq!(prev(&store, &[points(), colors()], t(4)), Some(t(3)));
        assert_eq!(prev(&store, &[points()], t(4)), Some(t(2)));
        assert_eq!(prev(&store, &[colors()], t(4)), Some(t(3)));
        assert_eq!(prev(&store, &[colors()], t(1)), None);
        assert_eq!(prev(&store, &[points()], t(0)), None);
        assert_eq!(prev(&store, &[], t(4)), None);
    }

    #[test]
    fn stepping_reaches_but_never_passes_the_time_bounds() {
        let mut store = test_store();
        let chunk = Chunk::builder("points")
            .with_archetype_auto_row(
                TimePoint::from([(timeline(), TimeInt::MIN.as_i64())]),
                &MyPoints::new([MyPoint::new(0.0, 0.0)]),
            )
            .with_archetype_auto_row(
                TimePoint::from([(timeline(), TimeInt::MAX.as_i64())]),
                &MyPoints::new([MyPoint::new(1.0, 0.0)]),
            )
            .build()
            .unwrap();
        insert(&mut store, chunk);

        assert_eq!(next(&store, &[points()], t(4)), Some(TimeInt::MAX));
        assert_eq!(prev(&store, &[points()], t(0)), Some(TimeInt::MIN));
        assert_eq!(next(&store, &[points()], TimeInt::MAX), None);
        assert_eq!(prev(&store, &[points()], TimeInt::MIN), None);
    }

    #[test]
    fn static_values_hide_temporal_events() {
        let mut store = test_store();
        let chunk = Chunk::builder("points")
            .with_archetype_auto_row(
                TimePoint::default(),
                &MyPoints::new([MyPoint::new(2.0, 0.0)]),
            )
            .build()
            .unwrap();
        insert(&mut store, chunk);

        assert_eq!(next(&store, &[points()], TimeInt::MIN), None);
        assert_eq!(prev(&store, &[points()], TimeInt::MAX), None);
    }

    #[test]
    fn stepping_an_unknown_entity_or_timeline_finds_nothing() {
        let store = test_store();

        assert_eq!(
            next_time_for_components(&store, timeline().name(), &"nope".into(), &[points()], t(0),),
            None
        );
        assert_eq!(
            next_time_for_components(
                &store,
                &TimelineName::from_static_str("nope"),
                &"points".into(),
                &[points()],
                t(0),
            ),
            None
        );
    }
}
