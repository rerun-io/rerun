use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange, Timeline};
use re_data_store::{ExtraQueryHistory, VisibleHistory};
use re_log_types::{EntityPath, RowId};
use re_query::query_archetype;
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{AnyQuery, Caches};

// ---

/// Generates a function to cache a query with N PoV components and M other components.
macro_rules! impl_query_cached_povN_compM {
    (impl $name:ident using $add_name:ident and $merge_name:ident with povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => {
        /// Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`].
        pub fn $name<'a, const N: usize, A, $($pov,)+ $($comp,)* F>(
            store: &'a DataStore,
            query: &AnyQuery,
            entity_path: &'a EntityPath,
            mut f: F,
        )
        where
            A: Archetype + 'a,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
            F: FnMut(
                &mut dyn Iterator<
                    Item = (
                        &(TimeInt, RowId),
                        &[InstanceKey],
                        $(&[$pov],)+
                        $(&[Option<$comp>],)*
                    ),
                >,
            ),
        {
            re_tracing::profile_scope!("query_cached_archetype_povN_compM", A::name());

            match &query {
                AnyQuery::LatestAtQuery(query) => {
                    Caches::with_latest_at::<A, _, _>(
                        store.id().clone(),
                        entity_path.clone(),
                        query,
                        |mut cache| {
                            // TODO: this should be a manually built scope so we can put the result of the cache hit
                            re_tracing::profile_scope!("latest_at");

                             let entry = cache.entry(query.at).or_default();
                            // NOTE: Implicitly dropping the write guard here: the LatestAtCache is
                            // free once again!

                            // let arch_view = query_archetype::<A>(store, &query, entity_path).ok(); // TODO

                            if entry.is_empty() {
                                // TODO: i already forgot whether we can use this on the web
                                let now = std::time::Instant::now();
                                let mut new_entries = 0u64;
                                let arch_view = query_archetype::<A>(store, &query, entity_path).ok(); // TODO
                                if let Some(arch_view) = arch_view {
                                    let time = query.at; // TODO: wait what? we need the real one!!!
                                    entry.$add_name::<A, $($pov,)+ $($comp,)*>(time, &arch_view);
                                    new_entries += 1;
                                }

                                if new_entries > 0 {
                                    let elapsed = now.elapsed();
                                    ::re_log::trace!(
                                        "cached {new_entries} new entries in {elapsed:?} ({:0.3} entries/s)",
                                        new_entries as f64 / elapsed.as_secs_f64()
                                    );
                                }
                            }

                            let mut it = itertools::izip!(
                                entry.iter_times(),
                                entry.iter_instance_keys(),
                                $(entry.iter_component::<$pov>(),)+
                                $(entry.iter_component_opt::<$comp>(),)*
                            );

                            f(&mut it);
                        }
                    )
                },
            }
        }
    };
    (impl $name:ident using $add_name:ident and $merge_name:ident with povs=[$($pov:ident)+]) => {
        impl_query_cached_povN_compM!(impl $name using $add_name and $merge_name with povs=[$($pov)+] comps=[]);
    };
}

impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1   using add_pov1   and merge_pov1
        with povs=[R1]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp1 using add_pov1_comp1 and merge_pov1_comp1
        with povs=[R1] comps=[O1]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp2 using add_pov1_comp2 and merge_pov1_comp2
        with povs=[R1] comps=[O1 O2]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp3 using add_pov1_comp3 and merge_pov1_comp3
        with povs=[R1] comps=[O1 O2 O3]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp4 using add_pov1_comp4 and merge_pov1_comp4
        with povs=[R1] comps=[O1 O2 O3 O4]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp5 using add_pov1_comp5 and merge_pov1_comp5
        with povs=[R1] comps=[O1 O2 O3 O4 O5]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp6 using add_pov1_comp6 and merge_pov1_comp6
        with povs=[R1] comps=[O1 O2 O3 O4 O5 O6]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp7 using add_pov1_comp7 and merge_pov1_comp7
        with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp8 using add_pov1_comp8 and merge_pov1_comp8
        with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7 O8]);
impl_query_cached_povN_compM!(
    impl query_cached_archetype_pov1_comp9 using add_pov1_comp9 and merge_pov1_comp9
        with povs=[R1] comps=[O1 O2 O3 O4 O5 O6 O7 O8 O9]);
