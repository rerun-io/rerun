use smallvec::smallvec;

use re_data_store::{DataStore, TimeInt, TimeRange};
use re_log_types::{build_frame_nr, DataCell, DataCellRow, DataRow, EntityPath, RowId};
use re_query::range_archetype;
use re_types::{
    archetypes::Points2D,
    components::{Color, InstanceKey, Position2D},
};
use re_types_core::Loggable as _;

#[test]
fn simple_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint1, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(10.0, 20.0), Position2D::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌─────────────┬───────────┬──────────────┐
    // │ InstanceKey ┆ Point2D   ┆ Color        │
    // ╞═════════════╪═══════════╪══════════════╡
    // │ 0           ┆ {1.0,2.0} ┆ null         │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1           ┆ {3.0,4.0} ┆ 4278190080   │
    // └─────────────┴───────────┴──────────────┘
    //
    // Frame #323:
    // ┌─────────────┬──────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D      ┆ Color           │
    // ╞═════════════╪══════════════╪═════════════════╡
    // │ 0           ┆ {10.0,20.0}  ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1           ┆ {30.0,40.0}  ┆ null            │
    // └─────────────┴──────────────┴─────────────────┘

    {
        // Frame #123

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![Some(Color::from_rgb(255, 0, 0)), None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Frame #123

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors: Vec<Option<Color>> = vec![None, None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![Some(Color::from_rgb(255, 0, 0)), None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
}

#[test]
fn timeless_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
        let mut row =
            DataRow::from_cells1(RowId::new(), ent_path.clone(), timepoint1, 2, &positions)
                .unwrap();
        row.compute_all_size_bytes();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), [], 2, &positions).unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances.clone(), colors.clone()),
        )
        .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            [],
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances.clone(), colors.clone()),
        )
        .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(10.0, 20.0), Position2D::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, &positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), [], 2, &positions).unwrap();
        store.insert_row(&row).unwrap();
    }

    // ┌───────────┬──────────┬────────┬─────────────────┬────────────────────┬────────────────────┬────────────────────────────┐
    // │ insert_id ┆ frame_nr ┆ entity ┆ Color ┆ InstanceKey ┆ rerun.row_id       ┆ Point2D              │
    // ╞═══════════╪══════════╪════════╪═════════════════╪════════════════════╪════════════════════╪════════════════════════════╡
    // │ 2         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302243… ┆ [{1.0,2.0}, {3.0,4.0}]     │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 4         ┆ null     ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302246… ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 8         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302249… ┆ [{10.0,20.0}, {30.0,40.0}] │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1         ┆ 123      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302236… ┆ [{1.0,2.0}, {3.0,4.0}]     │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 3         ┆ 123      ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302245… ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 5         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302247… ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 6         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302248… ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 7         ┆ 323      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302248… ┆ [{10.0,20.0}, {30.0,40.0}] │
    // └───────────┴──────────┴────────┴─────────────────┴────────────────────┴────────────────────┴────────────────────────────┘

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Frame #123

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![Some(Color::from_rgb(255, 0, 0)), None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will fall back to timeless data!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #122:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Frame #122 (all timeless)

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(122), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );

        // Frame #123 (partially timeless)

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[2];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![Some(Color::from_rgb(255, 0, 0)), None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }

    // --- Third test: `[-inf, +inf]` ---

    let query =
        re_data_store::RangeQuery::new(timepoint1[0].0, TimeRange::new(TimeInt::MIN, TimeInt::MAX));

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Timeless #1:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Timeless #2:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Timeless #1

        let arch_view = &results[0];
        let time = arch_view.data_time();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors: Vec<Option<Color>> = vec![None, None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(None, time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );

        // Timeless #2

        let arch_view = &results[1];
        let time = arch_view.data_time();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(None, time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );

        // Frame #123 (partially timeless)

        let arch_view = &results[2];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[3];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![Some(Color::from_rgb(255, 0, 0)), None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
}

#[test]
fn simple_splatted_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint1, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![Color::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with a splatted instance
        let color_instances = vec![InstanceKey::SPLAT];
        let colors = vec![Color::from_rgb(0, 255, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some Positions with implicit instances
        let positions = vec![Position2D::new(10.0, 20.0), Position2D::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), ent_path.clone(), timepoint3, 2, positions)
                .unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    // └────────────────────┴───────────────┴─────────────────┘

    assert_eq!(results.len(), 2);

    {
        // Frame #123

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }

    {
        // Frame #323

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![
            Some(Color::from_rgb(0, 255, 0)),
            Some(Color::from_rgb(0, 255, 0)),
        ];

        let df = arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap();
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(&expected, &df);
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let arch_views =
        range_archetype::<Points2D, { Points2D::NUM_COMPONENTS }>(&store, &query, &ent_path);

    let results = arch_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey ┆ Point2D ┆ Color │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Frame #123

        let arch_view = &results[0];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors: Vec<Option<Color>> = vec![None, None];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
    {
        // Frame #323

        let arch_view = &results[1];
        let time = arch_view.data_time().unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(10.0, 20.0)),
            Some(Position2D::new(30.0, 40.0)),
        ];
        let colors = vec![
            Some(Color::from_rgb(0, 255, 0)),
            Some(Color::from_rgb(0, 255, 0)),
        ];
        let expected = DataCellRow(smallvec![
            DataCell::from_native_sparse(instances),
            DataCell::from_native_sparse(positions),
            DataCell::from_native_sparse(colors)
        ]);

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        assert_eq!(
            &expected,
            &arch_view.to_data_cell_row_2::<Position2D, Color>().unwrap(),
        );
    }
}
