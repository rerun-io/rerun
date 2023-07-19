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
        2_770_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_bools".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_770_072, // expected_num_bytes
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
        2_920_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow("some_u64s".into(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        2_760_072, // expected_num_bytes
    );

    // utf8 (and more generally: dyn_binary)
    let mut cell = DataCell::from_native(
        [
            re_components::Label("hey".into()),
            re_components::Label("hey".into()),
            re_components::Label("hey".into()),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        3_170_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(re_components::Label::name(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        3_030_072, // expected_num_bytes
    );

    // struct
    let mut cell = DataCell::from_native(
        [
            re_components::Point2D::new(42.0, 666.0),
            re_components::Point2D::new(42.0, 666.0),
            re_components::Point2D::new(42.0, 666.0),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        5_340_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(re_components::Point2D::name(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        5_180_072, // expected_num_bytes
    );

    // struct + fixedsizelist
    let mut cell = DataCell::from_native(
        [
            re_components::Vec2D::from([42.0, 666.0]),
            re_components::Vec2D::from([42.0, 666.0]),
            re_components::Vec2D::from([42.0, 666.0]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        4_160_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(re_components::Point2D::name(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        4_000_072, // expected_num_bytes
    );

    // variable list
    let mut cell = DataCell::from_native(
        [
            re_components::LineStrip2D::from(vec![[42.0, 666.0], [42.0, 666.0], [42.0, 666.0]]),
            re_components::LineStrip2D::from(vec![[42.0, 666.0], [42.0, 666.0], [42.0, 666.0]]),
            re_components::LineStrip2D::from(vec![[42.0, 666.0], [42.0, 666.0], [42.0, 666.0]]),
        ]
        .as_slice(),
    );
    cell.compute_size_bytes();
    expect(
        cell.clone(), //
        10_000,       // num_rows
        6_200_072,    // expected_num_bytes
    );
    expect(
        DataCell::from_arrow(re_components::Point2D::name(), cell.to_arrow().sliced(1, 1)),
        10_000,    // num_rows
        5_640_072, // expected_num_bytes
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

    // This test uses an artificial enum type to test the union serialization.
    // The transform type does *not* represent our current transform representation.

    // --- Dense ---

    #[derive(Clone, Debug, PartialEq, ArrowField, ArrowSerialize, ArrowDeserialize)]
    #[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
    #[arrow_field(type = "dense")]
    enum DenseTransform {
        Unknown,
        Transform3D(re_components::Transform3DRepr),
        Pinhole(re_components::Pinhole),
    }

    impl re_log_types::LegacyComponent for DenseTransform {
        #[inline]
        fn legacy_name() -> re_log_types::ComponentName {
            "rerun.dense_transform".into()
        }
    }

    re_log_types::component_legacy_shim!(DenseTransform);

    // dense union (uniform)
    expect(
        DataCell::from_native(
            [
                DenseTransform::Unknown,
                DenseTransform::Unknown,
                DenseTransform::Unknown,
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        49_110_072, // expected_num_bytes
    );

    // dense union (varying)
    expect(
        DataCell::from_native(
            [
                DenseTransform::Unknown,
                DenseTransform::Transform3D(
                    re_components::TranslationAndMat3 {
                        translation: Some([10.0, 11.0, 12.0].into()),
                        matrix: [[13.0, 14.0, 15.0], [16.0, 17.0, 18.0], [19.0, 20.0, 21.0]].into(),
                    }
                    .into(),
                ),
                DenseTransform::Pinhole(re_components::Pinhole {
                    image_from_cam: [[21.0, 22.0, 23.0], [24.0, 25.0, 26.0], [27.0, 28.0, 29.0]]
                        .into(),
                    resolution: Some([123.0, 456.0].into()),
                }),
            ]
            .as_slice(),
        ),
        10_000,     // num_rows
        49_100_072, // expected_num_bytes
    );

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
        22_260_072, // expected_num_bytes
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
        21_810_072, // expected_num_bytes
    );
}
