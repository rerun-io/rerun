use itertools::{izip, Itertools as _};
use re_query2::PromiseResolver;
use re_query_cache2::Caches;
use re_types::{components::InstanceKey, Archetype};

use re_data_store::{DataStore, RangeQuery, StoreSubscriber as _, TimeInt, TimeRange};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimePoint,
};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() -> anyhow::Result<()> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, points)?;
        insert_and_react(&mut store, &mut caches, &row);

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 1, colors)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint2, 1, colors)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, points)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    Ok(())
}

// ---

fn insert_and_react(store: &mut DataStore, caches: &mut Caches, row: &DataRow) {
    caches.on_events(&[store.insert_row(row).unwrap()]);
}

fn query_and_compare(
    caches: &Caches,
    store: &DataStore,
    query: &RangeQuery,
    entity_path: &EntityPath,
) {
    re_log::setup_logging();

    let resolver = PromiseResolver::default();

    for _ in 0..3 {
        let cached = caches.range(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        // dbg!(&cached);

        let cached_all_points = cached
            .get_required(MyPoint::name())
            .unwrap()
            .to_dense::<MyPoint>(&resolver);
        let cached_all_points_indexed = cached_all_points.indexed(query.range());

        let cached_all_colors = cached
            .get_optional(MyColor::name())
            .to_sparse::<MyColor>(&resolver);
        let cached_all_colors_indexed = cached_all_colors.indexed(query.range());

        let expected = re_query2::range(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let expected_all_points = expected.get_required(MyPoint::name()).unwrap();
        let expected_all_points_indices = expected_all_points.indices();
        let expected_all_points_data = expected_all_points
            .to_dense::<MyPoint>(&resolver)
            .into_iter()
            .map(|batch| batch.flatten().unwrap())
            .collect_vec();
        let expected_all_points_indexed =
            izip!(expected_all_points_indices, expected_all_points_data);

        let expected_all_colors = expected.get_optional(MyColor::name());
        let expected_all_colors_indices = expected_all_colors.indices();
        let expected_all_colors_data = expected_all_colors
            .to_sparse::<MyColor>(&resolver)
            .into_iter()
            .map(|batch| batch.flatten().unwrap())
            .collect_vec();
        let expected_all_colors_indexed =
            izip!(expected_all_colors_indices, expected_all_colors_data);

        eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(
            expected_all_points_indexed
                .map(|(index, data)| (*index, data))
                .collect_vec(),
            cached_all_points_indexed
                .map(|(index, data)| (*index, data.to_vec()))
                .collect_vec(),
        );

        similar_asserts::assert_eq!(
            expected_all_colors_indexed
                .map(|(index, data)| (*index, data))
                .collect_vec(),
            cached_all_colors_indexed
                .map(|(index, data)| (*index, data.to_vec()))
                .collect_vec(),
        );
    }
}
