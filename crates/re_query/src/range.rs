use itertools::Itertools as _;
use re_data_store::{DataStore, RangeQuery};
use re_log_types::EntityPath;
use re_types_core::{Archetype, ComponentName};

use crate::{ArchetypeView, ComponentWithInstances};

// ---

/// Iterates over the rows of any number of components and their respective cluster keys, all from
/// the point-of-view of the required components, returning an iterator of [`ArchetypeView`]s.
///
/// An initial entity-view is yielded with the latest-at state at the start of the time range, if
/// there is any.
///
/// The iterator only ever yields entity-views iff a required component has changed: a change
/// affecting only optional components will not yield an entity-view.
/// However, the changes in those secondary components will be accumulated into the next yielded
/// entity-view.
///
/// This is a streaming-join: every yielded [`ArchetypeView`] will be the result of joining the latest
/// known state of all components, from their respective point-of-views.
///
/// ⚠ The semantics are subtle! See `examples/range.rs` for an example of use.
pub fn range_archetype<'a, A: Archetype + 'a, const N: usize>(
    store: &'a DataStore,
    query: &RangeQuery,
    ent_path: &'a EntityPath,
) -> impl Iterator<Item = ArchetypeView<A>> + 'a {
    re_tracing::profile_function!();

    // TODO(jleibs) this shim is super gross
    let components: [ComponentName; N] = A::all_components().into_owned().try_into().unwrap();

    let primary: ComponentName = A::required_components()[0];
    let cluster_key = store.cluster_key();

    // TODO(cmc): Ideally, we'd want to simply add the cluster and primary key to the `components`
    // array if they are missing, yielding either `[ComponentName; N+1]` or `[ComponentName; N+2]`.
    // Unfortunately this is not supported on stable at the moment, and requires
    // feature(generic_const_exprs) on nightly.
    //
    // The alternative to these assertions (and thus putting the burden on the caller), for now,
    // would be to drop the constant sizes all the way down, which would be way more painful to
    // deal with.
    assert!(components.contains(&cluster_key));
    assert!(components.contains(&primary));

    let cluster_col = components
        .iter()
        .find_position(|component| **component == cluster_key)
        .map(|(col, _)| col)
        .unwrap(); // asserted on entry
    let primary_col = components
        .iter()
        .find_position(|component| **component == primary)
        .map(|(col, _)| col)
        .unwrap(); // asserted on entry

    let mut state: Vec<_> = std::iter::repeat_with(|| None)
        .take(components.len())
        .collect();

    store
        .range(query, ent_path, components)
        .map(move |(data_time, row_id, mut cells)| {
            // NOTE: The unwrap cannot fail, the cluster key's presence is guaranteed
            // by the store.
            let instance_keys = cells[cluster_col].take().unwrap();
            let is_primary = cells[primary_col].is_some();
            let cwis = cells
                .into_iter()
                .map(|cell| {
                    cell.map(|cell| {
                        (
                            row_id,
                            ComponentWithInstances {
                                instance_keys: instance_keys.clone(), /* shallow */
                                values: cell,
                            },
                        )
                    })
                })
                .collect::<Vec<_>>();
            (data_time, is_primary, cwis)
        })
        .filter_map(move |(data_time, is_primary, cwis)| {
            for (i, cwi) in cwis
                .into_iter()
                .enumerate()
                .filter(|(_, cwi)| cwi.is_some())
            {
                state[i] = cwi;
            }

            // We only yield if the primary component has been updated!
            is_primary.then(|| {
                let (row_id, _) = state[primary_col].clone().unwrap(); // shallow

                let components: Vec<_> = state
                    .clone()
                    .into_iter()
                    .filter_map(|cwi| cwi.map(|(_, cwi)| cwi))
                    .collect();

                ArchetypeView::from_components(data_time, row_id, components)
            })
        })
}
