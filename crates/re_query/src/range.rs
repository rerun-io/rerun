use std::collections::BTreeMap;

use itertools::Itertools as _;
use re_arrow_store::{DataStore, LatestAtQuery, RangeQuery, RowIndex, TimeInt};
use re_log_types::{field_types::Instance, msg_bundle::Component, ComponentName, ObjPath};

use crate::{
    get_component_with_instances, query_entity_with_primary, ComponentWithInstances, EntityView,
    QueryError,
};

// ---

// TODO
pub fn range_entity_with_primary<'a>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a ObjPath,
    primary: ComponentName,
    components: &'a [ComponentName],
) -> impl Iterator<Item = (TimeInt, EntityView)> + 'a {
    let cluster_key = store.cluster_key();

    let mut state: Vec<_> = std::iter::repeat_with(|| None)
        .take(components.len() + 1) // +1 for primary
        .collect();
    let mut iters: Vec<_> = std::iter::repeat_with(|| None)
        .take(components.len() + 1) // +1 for primary
        .collect();

    let latest_time = query.range.min.as_i64().checked_sub(1).map(Into::into);

    if let Some(latest_time) = latest_time {
        // Fetch the latest data for every single component from their respective point-of-views,
        // this will allow us to build up the initial state and send an initial latest-at
        // entity-view if needed.
        for (i, primary) in std::iter::once(&primary)
            .chain(components.iter())
            .enumerate()
        {
            let cwi = get_component_with_instances(
                store,
                &LatestAtQuery::new(query.timeline, latest_time),
                ent_path,
                *primary,
            );
            state[i] = cwi.ok();
        }
    }

    // TODO
    // Iff the primary component has some initial state, then we want to be sending an initial
    // entity-view.
    let ent_view_latest = if let (Some(latest_time), Some(cwi_prim)) = (latest_time, &state[0]) {
        let ent_view = EntityView {
            primary: cwi_prim.clone(),
            components: components
                .iter()
                .copied()
                .zip(state.iter().skip(1).cloned())
                .filter_map(|(component, cwi)| cwi.map(|cwi| (component, cwi)))
                .collect(),
        };
        Some((latest_time, ent_view))
    } else {
        None
    };

    // Now let's create the actual range iterators, one for each component / point-of-view.
    for (i, component) in std::iter::once(primary)
        .chain(components.iter().copied())
        .enumerate()
    {
        let components = [cluster_key, component];

        let it = store.range(query, ent_path, component, components).map(
            move |(time, idx_row_nr, comp_row_nrs)| {
                let mut results = store.get(&components, &comp_row_nrs);
                (
                    i,
                    time,
                    idx_row_nr,
                    ComponentWithInstances {
                        name: component,
                        instance_keys: results[0].take(),
                        values: results[1].take().unwrap(), // TODO
                    },
                )
            },
        );

        iters[i] = Some(it);
    }

    ent_view_latest.into_iter().chain(
        iters
            .into_iter()
            .map(Option::unwrap)
            .kmerge_by(|(_, time1, idx_row_nr1, _), (_, time2, idx_row_nr2, _)| {
                // Merge earlier rows first, and tiebreak on the actual bucket index row
                // number if necessary!
                (time1, idx_row_nr1) < (time2, idx_row_nr2)
            })
            .filter_map(move |(i, time, _, cwi)| {
                state[i] = Some(cwi);

                // We only yield if the primary component changes!
                (i == 0).then(|| {
                    let ent_view = EntityView {
                        primary: state[0].clone().unwrap(), // TODO
                        components: components
                            .iter()
                            .zip(state.iter().skip(1).cloned())
                            .filter_map(|(component, cwi)| cwi.map(|cwi| (*component, cwi)))
                            .collect(),
                    };
                    (time, ent_view)
                })
            }),
    )
}
