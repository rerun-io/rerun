use itertools::Itertools;
use nohash_hasher::IntSet;
use re_data_store::{DataStore, LatestAtQuery, RangeQuery};
use re_log_types::{DataCell, EntityPath, RowId};
use re_types_core::ComponentName;

use crate::LatestAtResults;

// ---

// TODO: yielder is probably a bad idea -- back to pov?

// TODO
// TODO: i hate that const usize so much
pub fn range<'a, const N: usize>(
    store: &'a DataStore,
    query: &RangeQuery,
    entity_path: &EntityPath,
    yielder_name: impl Into<ComponentName>,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> impl Iterator<Item = LatestAtResults> + 'a {
    re_tracing::profile_function!(entity_path.to_string());

    let yielder_name = yielder_name.into();
    let component_names: IntSet<ComponentName> =
        component_names.into_iter().chain([yielder_name]).collect();

    assert_eq!(N, component_names.len(), "{component_names:?}");
    let components: [ComponentName; N] = component_names
        .iter()
        .copied()
        .collect_vec()
        .try_into()
        .unwrap();

    let yielder_col = component_names
        .iter()
        .find_position(|component_name| **component_name == yielder_name)
        .map(|(col, _)| col)
        .unwrap(); // we re-insert it ourselves

    let mut state: [Option<DataCell>; N] = [(); N].map(|_| None);

    // NOTE: This will return none for `TimeInt::Min`, i.e. range queries that start infinitely far
    // into the past don't have a latest-at state!
    let query_time = query.range.min.as_i64().checked_sub(1).map(Into::into);

    let mut cells_latest = None;
    if let Some(query_time) = query_time {
        let mut cells_latest_raw: [Option<DataCell>; N] = [(); N].map(|_| None);

        // Fetch the latest data for every single component from their respective point-of-views,
        // this will allow us to build up the initial state.
        for (i, component_name) in components.iter().copied().enumerate() {
            cells_latest_raw[i] = store
                .latest_at(
                    &LatestAtQuery::new(query.timeline, query_time),
                    entity_path,
                    component_name,
                    &[component_name],
                )
                .map(|(_, _, mut cells)|
                    // - `cells[0]` is guaranteed to exist since we passed `&[component_name]`
                    // - `cells[0]` is guaranteed to be non-null, otherwise the whole result would be null
                    cells[0].take().unwrap());
        }

        cells_latest = Some(cells_latest_raw);
    }

    cells_latest
        .into_iter()
        // NOTE: `false` here means we will _not_ yield the latest-at state as an actual
        // ArchetypeView!
        // That is a very important detail: for overlapping range queries to be correct in a
        // multi-tenant cache context, we need to make sure to inherit the latest-at state
        // from T-1, while also making sure to _not_ yield the view that comes with that state.
        //
        // Consider e.g. what happens when one system queries for `range(10, 20)` while another
        // queries for `range(9, 20)`: the data at timestamp `10` would differ because of the
        // statefulness of range queries!
        .map(move |cells| (query_time, RowId::ZERO, false, cells))
        .chain(store.range(query, entity_path, components).map(
            move |(data_time, row_id, cells)| {
                let is_yielder = cells[yielder_col].is_some();
                (data_time, row_id, is_yielder, cells)
            },
        ))
        .filter_map(move |(data_time, row_id, is_yielder, cells)| {
            for (i, cell) in cells
                .into_iter()
                .enumerate()
                .filter(|(_, cell)| cell.is_some())
            {
                state[i] = cell;
            }

            // We only yield if the yielder component has been updated!
            is_yielder.then(|| LatestAtResults {
                max_data_time: data_time,
                max_row_id: row_id,
                components: state
                    .iter()
                    .enumerate()
                    .filter_map(|(i, cell)| cell.clone().map(|cell| (components[i], cell)))
                    .collect(),
            })
        })
}

// TODO: move back to e2e tests
#[cfg(test)]
mod tests {
    use re_data_store::DataStore;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{build_frame_nr, DataRow, RowId, TimeInt, TimeRange};
    use re_types_core::{Archetype as _, Loggable as _};

    use crate::IteratorExt as _;

    use super::*;

    #[test]
    fn simple_range() -> anyhow::Result<()> {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            re_types::components::InstanceKey::name(),
            Default::default(),
        );

        let ent_path: EntityPath = "point".into();

        let timepoint1 = [build_frame_nr(123.into())];
        {
            // Create some Positions with implicit instances
            let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
            let row = DataRow::from_cells1_sized(
                RowId::new(),
                ent_path.clone(),
                timepoint1,
                2,
                positions,
            )
            .unwrap();
            store.insert_row(&row).unwrap();

            let colors = vec![MyColor::from_rgb(255, 0, 0)];
            let row =
                DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint1, 1, colors)
                    .unwrap();
            store.insert_row(&row).unwrap();
        }

        let timepoint2 = [build_frame_nr(223.into())];
        {
            let colors = vec![MyColor::from_rgb(255, 0, 0)];
            let row =
                DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint2, 1, colors)
                    .unwrap();
            store.insert_row(&row).unwrap();
        }

        let timepoint3 = [build_frame_nr(323.into())];
        {
            // Create some Positions with implicit instances
            let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
            let row = DataRow::from_cells1_sized(
                RowId::new(),
                ent_path.clone(),
                timepoint3,
                2,
                positions,
            )
            .unwrap();
            store.insert_row(&row).unwrap();
        }

        // --- First test: `(timepoint1, timepoint3]` ---

        let query = re_data_store::RangeQuery::new(
            timepoint1[0].0,
            TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
        );
        let results_raw = crate::range::<{ MyPoints::NUM_COMPONENTS }>(
            &store,
            &query,
            &ent_path,
            MyPoint::name(),
            MyPoints::all_components().iter().copied(),
        )
        .collect_vec();

        {
            // Frame #323

            let results_raw = &results_raw[0];

            let time = results_raw.max_data_time.unwrap();
            assert_eq!(TimeInt::from(323), time);

            let expected_components: IntSet<ComponentName> =
                [MyPoint::name(), MyColor::name()].into_iter().collect();
            let got_components: IntSet<ComponentName> =
                results_raw.components.keys().copied().collect();
            similar_asserts::assert_eq!(expected_components, got_components);

            let expected_positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
            let expected_colors = vec![
                Some(MyColor::from_rgb(255, 0, 0)),
                Some(MyColor::from_rgb(255, 0, 0)),
            ];

            let color_iter = results_raw.iter_optional_sparse::<MyColor>()?;
            let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

            let (got_positions, got_colors): (Vec<_>, Vec<_>) = results_raw
                .iter_required_dense::<MyPoint>()?
                .clamped_zip(color_iter, color_default_fn)
                .unzip();

            similar_asserts::assert_eq!(expected_positions, got_positions);
            similar_asserts::assert_eq!(expected_colors, got_colors);
        }

        // --- Second test: `[timepoint1, timepoint3]` ---

        let query = re_data_store::RangeQuery::new(
            timepoint1[0].0,
            TimeRange::new(timepoint1[0].1, timepoint3[0].1),
        );

        let results_raw = crate::range::<{ MyPoints::NUM_COMPONENTS }>(
            &store,
            &query,
            &ent_path,
            MyPoint::name(),
            MyPoints::all_components().iter().copied(),
        )
        .collect_vec();

        // We expect this to generate the following `DataFrame`s:
        //
        // Frame #123:
        // ┌───────────────┬───────────┐
        // │ Point2D       ┆ MyColor   │
        // ╞═══════════════╪═══════════╡
        // │ {1.0,2.0}     ┆ null      │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
        // │ {3.0,4.0}     ┆ null      │
        // └───────────────┴───────────┘
        //
        // Frame #323:
        // ┌───────────────┬─────────────┐
        // │ Point2D       ┆ MyColor     │
        // ╞═══════════════╪═════════════╡
        // │ {10.0,20.0}   ┆ 4278190080  │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ {30.0,40.0}   ┆ 4278190080  │
        // └───────────────┴─────────────┘

        {
            // Frame #123

            let results_raw = &results_raw[0];

            let time = results_raw.max_data_time.unwrap();
            assert_eq!(TimeInt::from(123), time);

            let expected_components: IntSet<ComponentName> =
                std::iter::once(MyPoint::name()).collect();
            let got_components: IntSet<ComponentName> =
                results_raw.components.keys().copied().collect();
            similar_asserts::assert_eq!(expected_components, got_components);

            let expected_positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
            let expected_colors = vec![None, None];

            let color_iter = results_raw.iter_optional_sparse::<MyColor>()?;
            let color_default_fn = || None;

            let (got_positions, got_colors): (Vec<_>, Vec<_>) = results_raw
                .iter_required_dense::<MyPoint>()?
                .clamped_zip(color_iter, color_default_fn)
                .unzip();

            similar_asserts::assert_eq!(expected_positions, got_positions);
            similar_asserts::assert_eq!(expected_colors, got_colors);
        }
        {
            // Frame #323

            let results_raw = &results_raw[1];

            let time = results_raw.max_data_time.unwrap();
            assert_eq!(TimeInt::from(323), time);

            let expected_components: IntSet<ComponentName> =
                [MyPoint::name(), MyColor::name()].into_iter().collect();
            let got_components: IntSet<ComponentName> =
                results_raw.components.keys().copied().collect();
            similar_asserts::assert_eq!(expected_components, got_components);

            let expected_positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
            let expected_colors = vec![
                Some(MyColor::from_rgb(255, 0, 0)),
                Some(MyColor::from_rgb(255, 0, 0)),
            ];

            let color_iter = results_raw.iter_optional_sparse::<MyColor>()?;
            let color_default_fn = || None;

            let (got_positions, got_colors): (Vec<_>, Vec<_>) = results_raw
                .iter_required_dense::<MyPoint>()?
                .clamped_zip(color_iter, color_default_fn)
                .unzip();

            similar_asserts::assert_eq!(expected_positions, got_positions);
            similar_asserts::assert_eq!(expected_colors, got_colors);
        }

        Ok(())
    }
}
