use std::collections::BTreeMap;

use paste::paste;
use seq_macro::seq;

use re_data_store::{DataStore, RangeQuery, TimeInt};
use re_log_types::{EntityPath, RowId};
use re_types_core::{components::InstanceKey, Archetype, Component};

use crate::{CacheBucket, MaybeCachedComponentData};

// --- Data structures ---

/// Caches the results of `Range` queries.
#[derive(Default)]
pub struct RangeCache {
    pub buckets: BTreeMap<TimeInt, CacheBucket>,
    //
    // TODO: dedupe?
    // TODO: size stats?
    // TODO: timeless?
}

// --- Queries ---

macro_rules! impl_query_archetype_range {
    (for N=$N:expr, M=$M:expr => povs=[$($pov:ident)+] comps=[$($comp:ident)*]) => { paste! {
        #[doc = "Cached implementation of [`re_query::query_archetype`] and [`re_query::range_archetype`]"]
        #[doc = "(combined) for `" $N "` point-of-view components and `" $M "` optional components."]
        #[allow(non_snake_case)]
        pub fn [<query_archetype_range_pov$N _comp$M>]<'a, A, $($pov,)+ $($comp,)* F>(
            store: &'a DataStore,
            query: &RangeQuery,
            entity_path: &'a EntityPath,
            mut f: F,
        ) -> ::re_query::Result<()>
        where
            A: Archetype + 'a,
            $($pov: Component + Send + Sync + 'static,)+
            $($comp: Component + Send + Sync + 'static,)*
            F: FnMut(
                (
                    (Option<TimeInt>, RowId),
                    MaybeCachedComponentData<'_, InstanceKey>,
                    $(MaybeCachedComponentData<'_, $pov>,)+
                    $(MaybeCachedComponentData<'_, Option<$comp>>,)*
                ),
            ),
        {

            // TODO

            Ok(())

        } }
    };

    // TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
    // not that we care at the moment.
    (for N=1, M=$M:expr) => {
        seq!(COMP in 1..=$M {
            impl_query_archetype_range!(for N=1, M=$M => povs=[R1] comps=[#(C~COMP)*]);
        });
    };
}

seq!(NUM_COMP in 0..10 {
    impl_query_archetype_range!(for N=1, M=NUM_COMP);
});
