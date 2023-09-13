use re_log_types::{DataCell, DataRow, DataTable, RowId, SizeBytes as _, TableId, TimePoint};

#[test]
fn data_table_sizes_basics() {
    use arrow2::array::{BooleanArray, UInt64Array};
    use re_types::Loggable as _;

    fn expect(mut cell: DataCell, num_rows: usize, num_bytes: u64) {
        cell.compute_size_bytes();

        let row = DataRow::from_cells1(
            RowId::random(),
            "a/b/c",
            TimePoint::default(),
            cell.num_instances(),
            cell,
        );

        let table = DataTable::from_rows(
            TableId::random(),
            std::iter::repeat_with(|| row.clone()).take(num_rows),
        );
        assert_eq!(num_bytes, table.heap_size_bytes());

        let mut table = DataTable::from_arrow_msg(&table.to_arrow_msg().unwrap()).unwrap();
        table.compute_all_size_bytes();
        let num_bytes = table.heap_size_bytes();
        assert_eq!(num_bytes, table.heap_size_bytes());
    }

    // boolean
    let mut cell = DataCell::from_arrow(
        "some_bools".into(),
        BooleanArray::from(vec![Some(true), Some(false), Some(true)]).boxed(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        2_690_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_bools".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_690_064, // expected_num_bytes
    );

    // primitive
    let mut cell = DataCell::from_arrow(
        "some_u64s".into(),
        UInt64Array::from_vec(vec![1, 2, 3]).boxed(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        2_840_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_u64s".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_680_064, // expected_num_bytes
    );

    // utf8 (and more generally: dyn_binary)
    let mut cell = DataCell::from_native(
        [
            re_types::components::Text("hey".into()),
            re_types::components::Text("hey".into()),
            re_types::components::Text("hey".into()),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        3_090_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            re_types::components::Text::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        2_950_064, // expected_num_bytes
    );

    //  fixedsizelist
    let mut cell = DataCell::from_native(
        [
            re_types::components::Position2D::from([42.0, 666.0]),
            re_types::components::Position2D::from([42.0, 666.0]),
            re_types::components::Position2D::from([42.0, 666.0]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        4_080_064,
    );
    expect(
        DataCell::from_arrow(
            re_types::components::Position2D::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000, // num_rows
        3_920_064,
    );

    // variable list
    let mut cell = DataCell::from_native(
        [
            re_types::components::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
            re_types::components::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
            re_types::components::LineStrip2D::from(vec![
                [42.0, 666.0],
                [42.0, 666.0],
                [42.0, 666.0],
            ]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        6_120_064,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(
            re_types::components::Position2D::name(),
            cell.to_arrow().sliced(1, 1),
        ),
        10_000,    // num_rows
        5_560_064, // expected_num_bytes
    );
}

#[test]
fn data_table_sizes_unions() {
    use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

    fn expect(mut cell: DataCell, num_rows: usize, num_bytes: u64) {
        cell.compute_size_bytes();

        let row = DataRow::from_cells1(
            RowId::random(),
            "a/b/c",
            TimePoint::default(),
            cell.num_instances(),
            cell,
        );

        let table = DataTable::from_rows(
            TableId::random(),
            std::iter::repeat_with(|| row.clone()).take(num_rows),
        );
        assert_eq!(num_bytes, table.heap_size_bytes());

        let err_margin = (num_bytes as f64 * 0.01) as u64;
        let num_bytes_min = num_bytes;
        let num_bytes_max = num_bytes + err_margin;

        let mut table = DataTable::from_arrow_msg(&table.to_arrow_msg().unwrap()).unwrap();
        table.compute_all_size_bytes();
        let num_bytes = table.heap_size_bytes();
        assert!(
            num_bytes_min <= num_bytes && num_bytes <= num_bytes_max,
            "{num_bytes_min} <= {num_bytes} <= {num_bytes_max}"
        );
    }

    // --- Sparse ---

    #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[arrow_field(type = "sparse")]
    enum SparseTransform {
        Unknown,
        Pinhole(re_components::Pinhole),
    }

    impl re_log_types::LegacyComponent for SparseTransform {
        #[inline]
        fn legacy_name() -> re_log_types::ComponentName {
            "rerun.sparse_transform".into()
        }
    }

    re_log_types::component_legacy_shim!(SparseTransform);

    // sparse union (uniform)
    expect(
        DataCell::from_native(
            [
                SparseTransform::Unknown,
                SparseTransform::Unknown,
                SparseTransform::Unknown,
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        22_180_064, // expected_num_bytes
    );

    // sparse union (varying)
    expect(
        DataCell::from_native(
            [
                SparseTransform::Unknown,
                SparseTransform::Pinhole(re_components::Pinhole {
                    image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]]
                        .into(),
                    resolution: Some([123.0, 456.0].into()),
                }),
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        21_730_064, // expected_num_bytes
    );
}
