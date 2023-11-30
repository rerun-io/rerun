use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_data_store::{ExtraQueryHistory, VisibleHistory};
use re_log::trace;
use re_log_types::{EntityPath, RowId};
use re_query::{query_archetype, range_archetype};
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{AnyQuery, Caches, CACHES};

// ---

// TODO: should return QueryError somehow somewhere..?
macro_rules! impl_query_cached_rNoM {
    (impl $name:ident using $add_name:ident and $merge_name:ident with required=[$($r:ident)+] optional=[$($o:ident)*]) => {
        // TODO: doc
        pub fn $name<'a, const N: usize, A, $($r,)+ $($o,)* F>(
            store: &'a DataStore,
            query: &AnyQuery,
            entity_path: &'a EntityPath,
            mut f: F,
        )
        where
            A: Archetype + 'a,
            $($r: Component + Send + Sync + 'static,)+
            $($o: Component + Send + Sync + 'static,)*
            F: FnMut(
                &mut dyn Iterator<
                    Item = (
                        &(TimeInt, RowId),
                        &[InstanceKey],
                        $(&[$r],)+
                        $(&[Option<$o>],)*
                    ),
                >,
            ),
        {
            re_tracing::profile_scope!("query_cached_archetype_rNoM", A::name());

            match &query {
                AnyQuery::LatestAtQuery(query) => {
                    Caches::with_latest::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |mut cache| {
                            re_tracing::profile_scope!("point");

                            // TODO: actually we have the same exact issue here... query.at might
                            // or might not match the actual time of the result???
                            // we dont care though, it still matches what you get when you ask for that time
                            // and that's what matters.
                            let cache = cache.entry(query.at).or_default();
                            // NOTE: Implicitly dropping the write guard here: the PointCache is
                            // free once again!

                            // TODO: this needs a btreemap too so that we can store the result per
                            // actual time, not query time

                            if cache.is_empty() {
                                let now = std::time::Instant::now();
                                let mut new_entries = 0u64;
                                let arch_view = query_archetype::<A>(store, &query, entity_path).ok(); // TODO
                                if let Some(arch_view) = arch_view {
                                    let time = query.at; // TODO: wait what? we need the real one!!!
                                    cache.$add_name::<A, $($r,)+ $($o,)*>(time, &arch_view);
                                    new_entries += 1;
                                }

                                if new_entries > 0 {
                                    let elapsed = now.elapsed();
                                    trace!(
                                        "cached {new_entries} new entries in {elapsed:?} ({:0.3} entries/s)",
                                        new_entries as f64 / elapsed.as_secs_f64()
                                    );
                                }
                            }

                            let mut it = itertools::izip!(
                                cache.iter_times(),
                                cache.iter_instance_keys(),
                                $(cache.iter_component::<$r>(),)+
                                $(cache.iter_component_opt::<$o>(),)*
                            );

                            f(&mut it);
                        }
                    )
                },
                AnyQuery::RangeQuery(query) => {
                    Caches::with_range::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |mut range_cache| {
                            re_tracing::profile_scope!("range");

                            // TODO: when do we drop the guard?

                            {
                                re_tracing::profile_scope!("upsert");

                                // TODO: explain... maybe
                                let reduced_queries = [
                                    range_cache.compute_left_query(query),
                                    range_cache.compute_right_query(query),
                                ];

                                for reduced_query in reduced_queries.into_iter().flatten() {
                                    let query_time = reduced_query.range.min;
                                    let bucket_time = range_cache.bucket_time(query_time);
                                    let next_bucket_time = range_cache.next_bucket_time(query_time);

                                    let now = std::time::Instant::now();
                                    // TODO: do the current batch in a clean cache to avoid costly OOOs
                                    let mut tmp_cache = crate::CachedQueryResult::default();
                                    // let tmp_cache = range_cache.entry(bucket_time).or_default();
                                    let arch_views = range_archetype::<A, N>(store, &reduced_query, entity_path);
                                    let mut new_entries = 0u64;
                                    for (time, arch_view) in arch_views {
                                        let time = time.unwrap_or(TimeInt::MIN); // TODO: uh oh
                                        tmp_cache.$add_name::<A, $($r,)+ $($o,)*>(time, &arch_view);
                                        new_entries += 1;
                                    }

                                    // TODO: order of merge depends on their respective range maybe?
                                    // TODO: do we even handle sub-timestamp inserted at many different
                                    // spots at this point

                                    // TODO: so what time do we want to register the buckets with?
                                    // - query mind bound?
                                    // - or result min bound?

                                    // TODO: this shouldnt even exist then
                                    if let Some(time_range) = tmp_cache.time_range() {
                                        let elapsed = now.elapsed();
                                        trace!(
                                            "cached {new_entries} new entries @ {} in {elapsed:?} ({:0.3} entries/s)",
                                            reduced_query
                                                .timeline
                                                .format_time_range_utc(&time_range),
                                            new_entries as f64 / elapsed.as_secs_f64(),
                                        );
                                        // trace!("{:?}", tmp_cache.times.iter());
                                        range_cache.insert(time_range.min, tmp_cache);
                                    }

                                    // let cache = range_cache.entry(bucket_time).or_default();
                                    //
                                    // let now = std::time::Instant::now();
                                    // let mut new_entries = 0u64;
                                    // let arch_views = range_archetype::<A, N>(store, &reduced_query, entity_path);
                                    // for (time, arch_view) in arch_views {
                                    //     let time = time.unwrap_or(TimeInt::MIN);
                                    //     cache.$add_name::<A, $($r,)+ $($o,)*>(time, &arch_view);
                                    //     new_entries += 1;
                                    // }
                                    //
                                    // if new_entries > 0 {
                                    //     let elapsed = now.elapsed();
                                    //     trace!(
                                    //         "cached {new_entries} new entries @ {} (via {}) in {elapsed:?} ({:0.3} entries/s)",
                                    //         reduced_query
                                    //             .timeline
                                    //             .format_time_range_utc(&reduced_query.range),
                                    //         reduced_query
                                    //             .timeline
                                    //             .format_time_utc(bucket_time),
                                    //         new_entries as f64 / elapsed.as_secs_f64(),
                                    //     );
                                    // } else if cache.is_empty()  {
                                    //     // TODO: probably indicates a bug in left/right computation
                                    //     range_cache.remove(&bucket_time);
                                    // }
                                }
                            }

                            // TODO: how on earth are we going to test this thing jesus
                            // TODO: this is where we gotta merge

                            // TODO: gotta do the bucketing here -> just don't merge beyond a
                            // certain size... queries will need to adapt though.
                            // but first we need single bucket to work perfectly.

                            // TODO: should be impl on  RangeCache with usual macro bs
                            if true {
                                re_tracing::profile_scope!("merge", &format!("num_buckets={}", range_cache.len()));

                                // TODO: cheap, we keep buckets number low
                                let mut buckets: Vec<_> = std::mem::take(&mut range_cache.0).into_iter().collect();

                                // TODO: can be written more efficiently ofc
                                'compaction: loop {
                                    for i in 1..buckets.len() {
                                        let should_merge = {
                                            let (rhs_time, rhs_cache) = &buckets[i];
                                            let (lhs_time, lhs_cache) = &buckets[i - 1];
                                            // TODO: cache cannot ever overlap anyway, right?
                                            // TODO: configurable bucket size
                                            lhs_cache.len() < 10_000
                                                && !lhs_cache.overlaps(rhs_cache)
                                                && lhs_cache.connects_to(rhs_cache)
                                        };

                                        if should_merge {
                                            let (rhs_time, rhs_cache) = buckets.remove(i);
                                            let (lhs_time, lhs_cache) = &mut buckets[i - 1];
                                            lhs_cache.$merge_name::<A, $($r,)+ $($o,)*>(rhs_cache);
                                            continue 'compaction;
                                        }
                                    }
                                    break;
                                }

                                range_cache.0 = buckets.into_iter().collect();
                            }

                            trace!(
                                "queried: {}",
                                query.timeline.format_time_range_utc(&query.range)
                            );
                            let bucket_time_min = range_cache.bucket_time(query.range.min).unwrap_or(TimeInt::MIN);
                            let bucket_time_max = range_cache.bucket_time(query.range.max).unwrap_or(TimeInt::MAX);
                            for (bucket_time, cache) in range_cache.range(bucket_time_min..=bucket_time_max) {
                                trace!(
                                    "looking through bucket {} -> {}",
                                    query.timeline.format_time_utc(*bucket_time),
                                    query.timeline.format_time_range_utc(&cache.time_range().unwrap()),
                                );
                                let mut it = {
                                    itertools::izip!(
                                        cache.range_times(query.range),
                                        cache.range_instance_keys(query.range),
                                        $(cache.range_component::<$r>(query.range),)+
                                        $(cache.range_component_opt::<$o>(query.range),)*
                                    )
                                };

                                f(&mut it);
                            }

                            // let cache = range_cache.entry(query.range.min).or_default();
                            // let mut it = {
                            //     re_tracing::profile_scope!("build iter");
                            //     itertools::izip!(
                            //         cache.range_times(query.range),
                            //         cache.range_instance_keys(query.range),
                            //         $(cache.range_component::<$r>(query.range),)+
                            //         $(cache.range_component_opt::<$o>(query.range),)*
                            //     )
                            // };
                            //
                            // f(&mut it)
                        }
                    )
                }
            }
        }
    };
    (impl $name:ident using $add_name:ident and $merge_name:ident with required=[$($r:ident)+]) => {
        impl_query_cached_rNoM!(impl $name using $add_name and $merge_name with required=[$($r)+] optional=[]);
    };
}

impl_query_cached_rNoM!(
    impl query_cached_archetype_r1   using add_r1   and merge_r1
        with required=[R1]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o1 using add_r1o1 and merge_r1o1
        with required=[R1] optional=[O1]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o2 using add_r1o2 and merge_r1o2
        with required=[R1] optional=[O1 O2]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o3 using add_r1o3 and merge_r1o3
        with required=[R1] optional=[O1 O2 O3]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o4 using add_r1o4 and merge_r1o4
        with required=[R1] optional=[O1 O2 O3 O4]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o5 using add_r1o5 and merge_r1o5
        with required=[R1] optional=[O1 O2 O3 O4 O5]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o6 using add_r1o6 and merge_r1o6
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o7 using add_r1o7 and merge_r1o7
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o8 using add_r1o8 and merge_r1o8
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8]);
impl_query_cached_rNoM!(
    impl query_cached_archetype_r1o9 using add_r1o9 and merge_r1o9
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8 O9]);

// ---

macro_rules! impl_query_cached_with_history_rNoM {
    (impl $name:ident on top of $query_name:ident with required=[$($r:ident)+] optional=[$($o:ident)*]) => {
        // TODO: doc
        pub fn $name<'a, const N: usize, A, $($r,)+ $($o,)* F>(
            store: &'a DataStore,
            timeline: &'a Timeline,
            time: &'a TimeInt,
            history: &ExtraQueryHistory,
            ent_path: &'a EntityPath,
            f: F,
        )
        where
            A: Archetype + 'a,
            $($r: Component + Send + Sync + 'static,)+
            $($o: Component + Send + Sync + 'static,)*
            F: FnMut(
                &mut dyn Iterator<
                    Item = (
                        &(TimeInt, RowId),
                        &[InstanceKey],
                        $(&[$r],)+
                        $(&[Option<$o>],)*
                    ),
                >,
            ),
        {
            let visible_history = match timeline.typ() {
                re_log_types::TimeType::Time => history.nanos,
                re_log_types::TimeType::Sequence => history.sequences,
            };

            if !history.enabled || visible_history == VisibleHistory::OFF {
                let query = LatestAtQuery::new(*timeline, *time);
                $crate::$query_name::<N, A, $($r,)+ $($o,)* _>(
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            } else {
                let min_time = visible_history.from(*time);
                let max_time = visible_history.to(*time);
                let query = RangeQuery::new(*timeline, TimeRange::new(min_time, max_time));
                $crate::$query_name::<N, A, $($r,)+ $($o,)* _>(
                    store,
                    &query.clone().into(),
                    ent_path,
                    f,
                )
            }
        }
    };
    (impl $name:ident on top of $query_name:ident with required=[$($r:ident)+]) => {
        impl_query_cached_with_history_rNoM!(impl $name on top of $query_name with required=[$($r)+] optional=[]);
    };
}

impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1   on top of query_cached_archetype_r1
        with required=[R1]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o1 on top of query_cached_archetype_r1o1
        with required=[R1] optional=[O1]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o2 on top of query_cached_archetype_r1o2
        with required=[R1] optional=[O1 O2]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o3 on top of query_cached_archetype_r1o3
        with required=[R1] optional=[O1 O2 O3]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o4 on top of query_cached_archetype_r1o4
        with required=[R1] optional=[O1 O2 O3 O4]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o5 on top of query_cached_archetype_r1o5
        with required=[R1] optional=[O1 O2 O3 O4 O5]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o6 on top of query_cached_archetype_r1o6
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o7 on top of query_cached_archetype_r1o7
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o8 on top of query_cached_archetype_r1o8
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8]);
impl_query_cached_with_history_rNoM!(
    impl query_cached_archetype_with_history_r1o9 on top of query_cached_archetype_r1o9
        with required=[R1] optional=[O1 O2 O3 O4 O5 O6 O7 O8 O9]);
