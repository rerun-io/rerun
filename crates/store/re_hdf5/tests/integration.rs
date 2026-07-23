//! Integration tests for `re_hdf5`.
//!
//! Fixtures are written in-test with `hdf5-pure`'s own writer (no committed
//! binaries, no h5py needed), except the libhdf5-compat test which reads a
//! committed h5py-written asset to guard against round-trip bias.

#![expect(clippy::unwrap_used)]

use arrow::array::{
    Array as _, FixedSizeListArray, Float32Array, Float64Array, Int64Array, ListArray, StringArray,
    StructArray, UInt8Array,
};
use hdf5_pure::{
    AttrValue, CharacterSet, CompoundTypeBuilder, Datatype, FileBuilder, StringPadding,
};
use itertools::Itertools as _;

use re_chunk::{Chunk, EntityPath};
use re_hdf5::{DatasetDtype, Hdf5Config, Hdf5Error, IndexColumn, IndexType, TimeUnit};
use re_log_types::TimeType;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn write_h5(build: impl FnOnce(&mut FileBuilder)) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.h5");
    let mut builder = FileBuilder::new();
    build(&mut builder);
    builder.write(&path).unwrap();
    (dir, path)
}

fn load_chunks(path: &std::path::Path, config: &Hdf5Config) -> Vec<Chunk> {
    re_hdf5::load_hdf5(path, config)
        .unwrap()
        .try_collect()
        .unwrap()
}

fn find_chunk<'a>(chunks: &'a [Chunk], entity: &str) -> &'a Chunk {
    chunks
        .iter()
        .find(|chunk| chunk.entity_path() == &EntityPath::from(entity))
        .unwrap_or_else(|| panic!("no chunk for entity {entity}"))
}

fn flat_config() -> Hdf5Config {
    Hdf5Config {
        use_structs: false,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Dimensionality mapping
// ---------------------------------------------------------------------------

#[test]
fn dimensionality_mapping() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("s0").with_i64_data(&[7]).with_shape(&[]);
        b.create_dataset("v1").with_f64_data(&[0.0, 1.0, 2.0, 3.0]);
        b.create_dataset("m2")
            .with_f32_data(&(0..12_u8).map(f32::from).collect::<Vec<_>>())
            .with_shape(&[4, 3]);
        #[expect(clippy::cast_possible_truncation)]
        b.create_dataset("t3")
            .with_u8_data(&(0..16).map(|i| i as u8).collect::<Vec<_>>())
            .with_shape(&[4, 2, 2]);
    });

    let chunks = load_chunks(&path, &flat_config());
    assert_eq!(chunks.len(), 2);

    let data = chunks.iter().find(|chunk| !chunk.is_static()).unwrap();
    assert_eq!(data.entity_path(), &EntityPath::from("/"));
    assert_eq!(data.num_rows(), 4);
    assert_eq!(data.num_components(), 3);

    // 1-D → one scalar per row.
    let v1 = data.components().get_array("v1".into()).unwrap();
    assert_eq!(v1.len(), 4);
    let v1_values = v1.values().as_any().downcast_ref::<Float64Array>().unwrap();
    assert_eq!(v1_values.values(), &[0.0, 1.0, 2.0, 3.0]);

    // 2-D [N, K] → one FixedSizeList<K> per row.
    let m2 = data.components().get_array("m2".into()).unwrap();
    assert_eq!(m2.len(), 4);
    let m2_values = m2
        .values()
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .unwrap();
    assert_eq!(m2_values.value_length(), 3);
    assert_eq!(m2_values.len(), 4);
    let m2_inner = m2_values
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .unwrap();
    assert_eq!(m2_inner.len(), 12);

    // 3-D+ [N, d1, …] → one row-major blob (List) per row.
    let t3 = data.components().get_array("t3".into()).unwrap();
    assert_eq!(t3.len(), 4);
    let t3_values = t3.values().as_any().downcast_ref::<ListArray>().unwrap();
    assert_eq!(t3_values.len(), 4);
    assert_eq!(t3_values.value_length(0), 4); // 2*2 values per row
    let t3_inner = t3_values
        .values()
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(t3_inner.len(), 16);

    // 0-D → a single static value.
    let statics = chunks.iter().find(|chunk| chunk.is_static()).unwrap();
    assert_eq!(statics.entity_path(), &EntityPath::from("/"));
    assert_eq!(statics.num_rows(), 1);
    let s0 = statics.components().get_array("s0".into()).unwrap();
    let s0_values = s0.values().as_any().downcast_ref::<Int64Array>().unwrap();
    assert_eq!(s0_values.values(), &[7]);
}

#[test]
fn string_datasets() {
    // Fixed-length strings need the raw on-disk layout: 5 bytes per element,
    // null-padded.
    let fixed_dtype = Datatype::String {
        size: 5,
        padding: StringPadding::NullPad,
        charset: CharacterSet::Utf8,
    };
    let mut fixed_bytes = Vec::new();
    for value in ["alpha", "be", "ce"] {
        let mut element = value.as_bytes().to_vec();
        element.resize(5, 0);
        fixed_bytes.extend_from_slice(&element);
    }

    let (_dir, path) = write_h5(move |b| {
        b.create_dataset("fixed")
            .with_raw_data(fixed_dtype, fixed_bytes, 3);
        b.create_dataset("varlen")
            .with_vlen_strings(&["x", "yy", "zzz"]);
    });

    let chunks = load_chunks(&path, &flat_config());
    let data = find_chunk(&chunks, "/");
    assert_eq!(data.num_rows(), 3);

    for (component, expected) in [
        ("fixed", vec!["alpha", "be", "ce"]),
        ("varlen", vec!["x", "yy", "zzz"]),
    ] {
        let array = data.components().get_array(component.into()).unwrap();
        let values = array
            .values()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(values.iter().map(Option::unwrap).collect_vec(), expected);
    }
}

// ---------------------------------------------------------------------------
// Timelines
// ---------------------------------------------------------------------------

#[test]
fn index_column_types_and_units() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("ts").with_i64_data(&[1, 2, 3]);
        b.create_dataset("value").with_f64_data(&[1.0, 2.0, 3.0]);
    });

    for (index_type, expected_time_type, scale) in [
        (
            IndexType::Timestamp(TimeUnit::Nanoseconds),
            TimeType::TimestampNs,
            1,
        ),
        (
            IndexType::Timestamp(TimeUnit::Microseconds),
            TimeType::TimestampNs,
            1_000,
        ),
        (
            IndexType::Duration(TimeUnit::Milliseconds),
            TimeType::DurationNs,
            1_000_000,
        ),
        (
            IndexType::Timestamp(TimeUnit::Seconds),
            TimeType::TimestampNs,
            1_000_000_000,
        ),
        (IndexType::Sequence, TimeType::Sequence, 1),
    ] {
        let config = Hdf5Config {
            index_column: Some(IndexColumn {
                path: "/ts".into(),
                index_type,
            }),
            ..Default::default()
        };
        let chunks = load_chunks(&path, &config);
        assert_eq!(chunks.len(), 1);

        let data = &chunks[0];
        // The index is consumed: only `value` remains, as a bare component.
        assert_eq!(data.num_components(), 1);
        assert!(data.components().get_array("value".into()).is_some());

        // The timeline is named after the index dataset's leaf.
        let time_column = data.timelines().get(&"ts".into()).unwrap();
        assert_eq!(time_column.timeline().typ(), expected_time_type);
        assert_eq!(time_column.times_raw(), &[scale, 2 * scale, 3 * scale]);
        assert!(time_column.is_sorted());
    }
}

#[test]
fn float_seconds_index_preserves_subsecond_precision() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("time").with_f64_data(&[0.5, 1.5]);
        b.create_dataset("value").with_f64_data(&[1.0, 2.0]);
    });

    let config = Hdf5Config {
        index_column: Some(IndexColumn {
            path: "/time".into(),
            index_type: IndexType::Timestamp(TimeUnit::Seconds),
        }),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let time_column = chunks[0].timelines().get(&"time".into()).unwrap();

    // Scale-before-round: 0.5 s → 500_000_000 ns (truncating the float first
    // would yield 0 and 1_000_000_000).
    assert_eq!(time_column.times_raw(), &[500_000_000, 1_500_000_000]);
}

#[test]
fn index_in_nested_group_is_consumed() {
    let (_dir, path) = write_h5(|b| {
        let mut nav = b.create_group("nav");
        nav.create_dataset("stamp").with_i64_data(&[10, 20]);
        nav.create_dataset("heading").with_f64_data(&[0.1, 0.2]);
        b.add_group(nav.finish());
    });

    let config = Hdf5Config {
        index_column: Some(IndexColumn {
            path: "/nav/stamp".into(),
            index_type: IndexType::Sequence,
        }),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    assert_eq!(chunks.len(), 1);

    let nav = find_chunk(&chunks, "/nav");
    assert_eq!(nav.num_components(), 1);
    assert!(nav.components().get_array("heading".into()).is_some());
    assert!(nav.timelines().contains_key(&"stamp".into()));
}

#[test]
fn no_index_synthesizes_row_index() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("value").with_f64_data(&[1.0, 2.0, 3.0]);
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());
    let time_column = chunks[0].timelines().get(&"row_index".into()).unwrap();
    assert_eq!(time_column.timeline().typ(), TimeType::Sequence);
    assert_eq!(time_column.times_raw(), &[0, 1, 2]);
    assert!(time_column.is_sorted());
}

#[test]
fn unsorted_index_rows_are_stably_reordered() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("ts").with_i64_data(&[3, 1, 2]);
        b.create_dataset("value").with_f64_data(&[1.0, 2.0, 3.0]);
    });

    let config = Hdf5Config {
        index_column: Some(IndexColumn {
            path: "/ts".into(),
            index_type: IndexType::Sequence,
        }),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);

    // `Chunk::from_auto_row_ids` stably reorders each chunk's rows so its time
    // column is non-decreasing; the data rows stay aligned to their times.
    let time_column = chunks[0].timelines().get(&"ts".into()).unwrap();
    assert_eq!(time_column.times_raw(), &[1, 2, 3]);
    assert!(time_column.is_sorted());

    let value = chunks[0].components().get_array("value".into()).unwrap();
    let values = value
        .values()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(values.values(), &[2.0, 3.0, 1.0]);
}

#[test]
fn all_scalar_file_has_no_timeline() {
    let (_dir, path) = write_h5(|b| {
        b.set_attr("note", AttrValue::String("static only".into()));
        b.create_dataset("s").with_i64_data(&[3]).with_shape(&[]);
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());
    assert_eq!(chunks.len(), 2);
    assert!(chunks.iter().all(Chunk::is_static));
}

// ---------------------------------------------------------------------------
// Row windowing
// ---------------------------------------------------------------------------

#[test]
fn large_dataset_is_row_windowed() {
    const NUM_ROWS: usize = 2500; // 1024 + 1024 + 452

    #[expect(clippy::cast_precision_loss)]
    let values: Vec<f64> = (0..NUM_ROWS).map(|i| i as f64).collect();
    let (_dir, path) = write_h5(move |b| {
        b.create_dataset("value").with_f64_data(&values);
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());
    assert_eq!(chunks.len(), 3);
    assert_eq!(
        chunks.iter().map(Chunk::num_rows).collect_vec(),
        vec![1024, 1024, 452]
    );

    // Distinct chunk ids, same entity, contiguous aligned time windows — every
    // window inheriting the file-wide buffer's sortedness.
    assert_eq!(chunks.iter().map(Chunk::id).unique().count(), 3);
    let all_times: Vec<i64> = chunks
        .iter()
        .flat_map(|chunk| {
            assert_eq!(chunk.entity_path(), &EntityPath::from("/"));
            let time_column = chunk.timelines().get(&"row_index".into()).unwrap();
            assert!(time_column.is_sorted());
            time_column.times_raw().to_vec()
        })
        .collect();
    #[expect(clippy::cast_possible_wrap)]
    let expected: Vec<i64> = (0..NUM_ROWS as i64).collect();
    assert_eq!(all_times, expected);
}

#[test]
fn small_dataset_is_a_single_chunk() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("value").with_f64_data(&[1.0, 2.0]);
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].num_rows(), 2);
}

// ---------------------------------------------------------------------------
// Root group
// ---------------------------------------------------------------------------

/// A LIBERO-shaped file: sibling episode groups with *different* row counts,
/// plus file-level attributes above them.
fn write_two_episode_file() -> (tempfile::TempDir, std::path::PathBuf) {
    write_h5(|b| {
        b.set_attr("convention", AttrValue::String("opengl".into()));

        let mut demo_0 = b.create_group("demo_0");
        demo_0.set_attr("num_samples", AttrValue::I64(3));
        demo_0.create_dataset("t").with_i64_data(&[10, 20, 30]);
        let mut obs = demo_0.create_group("obs");
        obs.create_dataset("qpos")
            .with_f64_data(&[0.0, 0.1, 1.0, 1.1, 2.0, 2.1])
            .with_shape(&[3, 2]);
        demo_0.add_group(obs.finish());
        b.add_group(demo_0.finish());

        let mut demo_1 = b.create_group("demo_1");
        demo_1.create_dataset("t").with_i64_data(&[1, 2, 3, 4, 5]);
        b.add_group(demo_1.finish());
    })
}

#[test]
fn root_group_scopes_to_subtree() {
    let (_dir, path) = write_two_episode_file();

    // The whole file cannot be loaded: the sibling episodes disagree on rows.
    let err = re_hdf5::load_hdf5(&path, &Hdf5Config::default())
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RowAlignment { .. }), "{err}");

    // Scoped to one episode it loads, with the index resolved *relative* to
    // the root group and entity paths relative to it as well.
    let config = Hdf5Config {
        root_group: Some("/demo_0".into()),
        index_column: Some(IndexColumn {
            path: "t".into(),
            index_type: IndexType::Sequence,
        }),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);

    // demo_0's own attributes act as root attributes; the file-level
    // `convention` attribute lives *above* the root group and is not emitted.
    let props = find_chunk(&chunks, "/__hdf5_properties");
    assert!(props.components().get_array("num_samples".into()).is_some());
    assert!(
        !chunks
            .iter()
            .any(|chunk| chunk.components().get_array("convention".into()).is_some())
    );

    // `/demo_0/obs/qpos` emits at `/obs`, timed by the consumed `t` index.
    let obs = find_chunk(&chunks, "/obs");
    assert_eq!(obs.num_rows(), 3);
    assert!(obs.components().get_array("qpos".into()).is_some());
    let time_column = obs.timelines().get(&"t".into()).unwrap();
    assert_eq!(time_column.times_raw(), &[10, 20, 30]);

    assert_eq!(chunks.len(), 2);
}

#[test]
fn root_group_relative_ignore_and_prefix() {
    let (_dir, path) = write_two_episode_file();

    // `ignore_datasets` is root-relative, and `entity_path_prefix` still
    // applies on top (the multiplex-into-one-recording pattern).
    let config = Hdf5Config {
        root_group: Some("/demo_0".into()),
        ignore_datasets: vec!["obs".into()],
        entity_path_prefix: EntityPath::from("/demo_0"),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);

    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.entity_path().to_string().starts_with("/demo_0"))
    );
    let data = find_chunk(&chunks, "/demo_0");
    assert_eq!(data.num_rows(), 3);
    assert!(data.components().get_array("t".into()).is_some());
    assert!(
        !chunks
            .iter()
            .any(|chunk| { chunk.entity_path() == &EntityPath::from("/demo_0/obs") })
    );
}

#[test]
fn root_group_validation_errors() {
    let (_dir, path) = write_two_episode_file();

    let with_root = |root: &str| Hdf5Config {
        root_group: Some(root.into()),
        ..Default::default()
    };

    let err = re_hdf5::validate_layout(&path, &with_root("/nope"))
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RootGroupNotFound { .. }), "{err}");
    assert!(err.is_config_error());

    let err = re_hdf5::validate_layout(&path, &with_root("/demo_0/t"))
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RootGroupNotAGroup { .. }), "{err}");
    assert!(err.is_config_error());

    // An explicit `/` root group behaves exactly like `None`.
    let err = re_hdf5::validate_layout(&path, &with_root("/"))
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RowAlignment { .. }), "{err}");
}

// ---------------------------------------------------------------------------
// Alignment & ignore rules
// ---------------------------------------------------------------------------

#[test]
fn misaligned_datasets_error_and_ignore_resolves() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("a").with_f64_data(&[0.0, 1.0, 2.0, 3.0]);
        b.create_dataset("b")
            .with_f64_data(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    });

    let err = re_hdf5::load_hdf5(&path, &Hdf5Config::default())
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RowAlignment { .. }), "{err}");
    assert!(err.is_config_error());
    assert!(err.to_string().contains("/b (shape [6])"), "{err}");

    // `validate_layout` returns the same error eagerly, without reading data.
    let err = re_hdf5::validate_layout(&path, &Hdf5Config::default())
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::RowAlignment { .. }), "{err}");

    // Explicitly ignoring the offender resolves the mismatch.
    let config = Hdf5Config {
        ignore_datasets: vec!["/b".into()],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].num_rows(), 4);
    assert!(chunks[0].components().get_array("a".into()).is_some());
}

#[test]
fn ignore_group_subtree() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("keep").with_f64_data(&[1.0, 2.0]);
        let mut g = b.create_group("g");
        g.set_attr("note", AttrValue::String("attr in ignored subtree".into()));
        g.create_dataset("x").with_f64_data(&[1.0, 2.0, 3.0]); // misaligned unless ignored
        b.add_group(g.finish());
    });

    let config = Hdf5Config {
        ignore_datasets: vec!["/g".into()],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);

    // Only the root data chunk: no `/g` data, and no `/g` attributes either.
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].entity_path(), &EntityPath::from("/"));

    // Ignoring the single dataset (not the group) keeps the group's attributes.
    let config = Hdf5Config {
        ignore_datasets: vec!["/g/x".into()],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    assert_eq!(chunks.len(), 2);
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.entity_path() == &EntityPath::from("/__hdf5_properties/g"))
    );
}

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

#[test]
fn attributes_become_static_components() {
    let (_dir, path) = write_h5(|b| {
        b.set_attr("version", AttrValue::I64(1));
        let mut g = b.create_group("g");
        g.set_attr("freq", AttrValue::F64(30.0));
        g.set_attr("vec", AttrValue::F64Array(vec![1.0, 2.0, 3.0]));
        g.create_dataset("d")
            .with_f64_data(&[1.0])
            .set_attr("unit", AttrValue::String("m".into()));
        b.add_group(g.finish());
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());

    // Root attributes land on `__hdf5_properties` itself.
    let root_props = find_chunk(&chunks, "/__hdf5_properties");
    assert!(root_props.is_static());
    let version = root_props.components().get_array("version".into()).unwrap();
    let version_values = version
        .values()
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(version_values.values(), &[1]);

    // Attributes on `/g` mirror to `__hdf5_properties/g`, typed per the value.
    let g_props = find_chunk(&chunks, "/__hdf5_properties/g");
    assert!(g_props.is_static());
    let freq = g_props.components().get_array("freq".into()).unwrap();
    let freq_values = freq
        .values()
        .as_any()
        .downcast_ref::<Float64Array>()
        .unwrap();
    assert_eq!(freq_values.values(), &[30.0]);

    let vec_array = g_props.components().get_array("vec".into()).unwrap();
    assert_eq!(vec_array.len(), 1);
    let vec_values = vec_array
        .values()
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .unwrap();
    assert_eq!(vec_values.value_length(), 3);

    // Attributes on the dataset `/g/d` mirror to `__hdf5_properties/g/d`.
    let d_props = find_chunk(&chunks, "/__hdf5_properties/g/d");
    let unit = d_props.components().get_array("unit".into()).unwrap();
    let unit_values = unit
        .values()
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert_eq!(unit_values.value(0), "m");
}

#[test]
fn read_attributes_accessor() {
    let (_dir, path) = write_h5(|b| {
        b.set_attr("b_root", AttrValue::I64(2));
        b.set_attr("a_root", AttrValue::I64(1));
        let mut g = b.create_group("g");
        g.set_attr("freq", AttrValue::F64(30.0));
        g.create_dataset("d")
            .with_f64_data(&[1.0])
            .set_attr("unit", AttrValue::String("m".into()));
        b.add_group(g.finish());
    });

    // Sorted by name.
    let root_attrs = re_hdf5::read_attributes(&path, "/").unwrap();
    assert_eq!(
        root_attrs,
        vec![
            ("a_root".to_owned(), AttrValue::I64(1)),
            ("b_root".to_owned(), AttrValue::I64(2)),
        ]
    );

    let group_attrs = re_hdf5::read_attributes(&path, "/g").unwrap();
    assert_eq!(group_attrs, vec![("freq".to_owned(), AttrValue::F64(30.0))]);

    let dataset_attrs = re_hdf5::read_attributes(&path, "/g/d").unwrap();
    assert_eq!(
        dataset_attrs,
        vec![("unit".to_owned(), AttrValue::String("m".into()))]
    );

    let err = re_hdf5::read_attributes(&path, "/missing").err().unwrap();
    assert!(err.is_not_found(), "{err}");
}

// ---------------------------------------------------------------------------
// Struct packing
// ---------------------------------------------------------------------------

#[test]
fn use_structs_packs_group_datasets() {
    let (_dir, path) = write_h5(|b| {
        let mut g = b.create_group("g");
        g.create_dataset("a").with_f64_data(&[1.0, 2.0]);
        g.create_dataset("m")
            .with_f64_data(&[1.0, 2.0, 3.0, 4.0])
            .with_shape(&[2, 2]);
        b.add_group(g.finish());
    });

    // Struct mode: one `data` component whose struct fields keep each
    // dataset's per-row shape (scalar and FixedSizeList<2>).
    let chunks = load_chunks(&path, &Hdf5Config::default());
    let g = find_chunk(&chunks, "/g");
    assert_eq!(g.num_components(), 1);
    let data = g.components().get_array("data".into()).unwrap();
    let structs = data
        .values()
        .as_any()
        .downcast_ref::<StructArray>()
        .unwrap();
    assert_eq!(structs.num_columns(), 2);
    assert_eq!(structs.column_by_name("a").unwrap().len(), 2);
    let m_field = structs.column_by_name("m").unwrap();
    let m_field = m_field
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .unwrap();
    assert_eq!(m_field.value_length(), 2);

    // Flat mode: one component per dataset.
    let chunks = load_chunks(&path, &flat_config());
    let g = find_chunk(&chunks, "/g");
    assert_eq!(g.num_components(), 2);
    assert!(g.components().get_array("a".into()).is_some());
    assert!(g.components().get_array("m".into()).is_some());
}

#[test]
fn use_structs_single_dataset_carve_out() {
    let (_dir, path) = write_h5(|b| {
        let mut g = b.create_group("g");
        g.create_dataset("only").with_f64_data(&[1.0, 2.0]);
        b.add_group(g.finish());
    });

    // A single-dataset group emits a bare component, not a 1-field struct.
    let chunks = load_chunks(&path, &Hdf5Config::default());
    let g = find_chunk(&chunks, "/g");
    assert_eq!(g.num_components(), 1);
    assert!(g.components().get_array("only".into()).is_some());
}

#[test]
fn entity_path_prefix_is_applied() {
    let (_dir, path) = write_h5(|b| {
        b.set_attr("version", AttrValue::I64(1));
        let mut g = b.create_group("g");
        g.create_dataset("d").with_f64_data(&[1.0]);
        b.add_group(g.finish());
    });

    let config = Hdf5Config {
        entity_path_prefix: EntityPath::from("/world"),
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.entity_path() == &EntityPath::from("/world/g"))
    );
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.entity_path() == &EntityPath::from("/world/__hdf5_properties"))
    );
}

// ---------------------------------------------------------------------------
// Unsupported element types
// ---------------------------------------------------------------------------

#[test]
fn unsupported_dtype_is_skipped_and_exempt_from_alignment() {
    let compound_dtype = CompoundTypeBuilder::new().f64_field("x").build();

    let (_dir, path) = write_h5(move |b| {
        // 7 compound elements vs 3 f64 rows: would fail alignment if the
        // compound dataset were not excluded.
        b.create_dataset("c")
            .with_compound_data(compound_dtype, vec![0_u8; 7 * 8], 7);
        b.create_dataset("v").with_f64_data(&[1.0, 2.0, 3.0]);
    });

    let chunks = load_chunks(&path, &Hdf5Config::default());
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].num_rows(), 3);
    assert!(chunks[0].components().get_array("v".into()).is_some());
}

// ---------------------------------------------------------------------------
// Metadata accessors
// ---------------------------------------------------------------------------

#[test]
fn list_groups_and_datasets() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("time").with_f64_data(&[0.0, 1.0]);
        let mut observations = b.create_group("observations");
        #[expect(clippy::cast_possible_truncation)]
        observations
            .create_dataset("qpos")
            .with_u8_data(&(0..6).map(|i| i as u8).collect::<Vec<_>>())
            .with_shape(&[2, 3]);
        let mut images = observations.create_group("images");
        images.create_dataset("cam0").with_f32_data(&[1.0, 2.0]);
        observations.add_group(images.finish());
        b.add_group(observations.finish());
    });

    assert_eq!(
        re_hdf5::list_groups(&path, "/").unwrap(),
        vec!["/observations", "/observations/images"]
    );
    assert_eq!(
        re_hdf5::list_groups(&path, "/observations").unwrap(),
        vec!["/observations/images"]
    );

    let datasets = re_hdf5::list_datasets(&path, "/").unwrap();
    let described = datasets
        .iter()
        .map(|info| (info.path.as_str(), info.shape.clone(), info.dtype))
        .collect_vec();
    assert_eq!(
        described,
        vec![
            ("/time", vec![2], DatasetDtype::Float64),
            ("/observations/qpos", vec![2, 3], DatasetDtype::UInt8),
            ("/observations/images/cam0", vec![2], DatasetDtype::Float32),
        ]
    );

    let under_group = re_hdf5::list_datasets(&path, "/observations/images").unwrap();
    assert_eq!(under_group.len(), 1);
    assert_eq!(under_group[0].path, "/observations/images/cam0");

    let err = re_hdf5::list_groups(&path, "/missing").err().unwrap();
    assert!(err.is_not_found(), "{err}");
}

/// Test that our Display implementation prints dtype in numpy-style, like h5py (but unlike
/// hdf5-pure), matching community expectations.
#[test]
fn dtype_names_are_numpy_style() {
    let compound_dtype = CompoundTypeBuilder::new().f64_field("x").build();
    let (_dir, path) = write_h5(move |b| {
        b.create_dataset("bytes").with_u8_data(&[1, 2]);
        b.create_dataset("floats").with_f64_data(&[1.0, 2.0]);
        b.create_dataset("strings").with_vlen_strings(&["a", "b"]);
        b.create_dataset("compound")
            .with_compound_data(compound_dtype, vec![0_u8; 2 * 8], 2);
    });

    let dtypes: std::collections::HashMap<String, String> = re_hdf5::list_datasets(&path, "/")
        .unwrap()
        .into_iter()
        .map(|info| (info.path, info.dtype.to_string()))
        .collect();

    // Numpy-style names, not `DType`'s short `Display` names ("u8"/"f64").
    assert_eq!(dtypes["/bytes"], "uint8");
    assert_eq!(dtypes["/floats"], "float64");
    assert_eq!(dtypes["/strings"], "string");
    assert_eq!(dtypes["/compound"], "unsupported");
}

// ---------------------------------------------------------------------------
// Validation errors
// ---------------------------------------------------------------------------

#[test]
fn validate_layout_index_errors() {
    let (_dir, path) = write_h5(|b| {
        b.create_dataset("value").with_f64_data(&[1.0, 2.0]);
        b.create_dataset("matrix")
            .with_f64_data(&[1.0, 2.0, 3.0, 4.0])
            .with_shape(&[2, 2]);
        b.create_dataset("names").with_vlen_strings(&["a", "b"]);
    });

    let config_with_index = |index_path: &str| Hdf5Config {
        index_column: Some(IndexColumn {
            path: index_path.into(),
            index_type: IndexType::Sequence,
        }),
        ..Default::default()
    };

    let err = re_hdf5::validate_layout(&path, &config_with_index("/missing"))
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::IndexNotFound { .. }), "{err}");
    assert!(err.is_config_error());

    let err = re_hdf5::validate_layout(&path, &config_with_index("/matrix"))
        .err()
        .unwrap();
    assert!(
        matches!(err, Hdf5Error::IndexNotOneDimensional { .. }),
        "{err}"
    );

    let err = re_hdf5::validate_layout(&path, &config_with_index("/names"))
        .err()
        .unwrap();
    assert!(matches!(err, Hdf5Error::IndexNotNumeric { .. }), "{err}");

    assert!(re_hdf5::validate_layout(&path, &config_with_index("/value")).is_ok());
}

// ---------------------------------------------------------------------------
// libhdf5 compatibility
// ---------------------------------------------------------------------------

/// Reads a committed h5py-written fixture — every other test round-trips
/// through `hdf5-pure`'s own writer, which would hide writer-symmetric parser
/// bugs and never exercises libhdf5 idiosyncrasies (v1 symbol-table groups,
/// string padding, attribute encodings).
#[test]
fn reads_h5py_written_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/assets/h5py_compat.h5");
    assert!(
        path.exists(),
        "missing git-lfs asset {path:?} — run `git lfs pull`"
    );

    let chunks = load_chunks(&path, &flat_config());

    let data = find_chunk(&chunks, "/observations");
    assert_eq!(data.num_rows(), 4);
    let qpos = data.components().get_array("qpos".into()).unwrap();
    let qpos_values = qpos
        .values()
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .unwrap();
    assert_eq!(qpos_values.value_length(), 3);

    let props = find_chunk(&chunks, "/__hdf5_properties");
    let version = props.components().get_array("version".into()).unwrap();
    let version_values = version
        .values()
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    assert_eq!(version_values.values(), &[2]);

    let attrs = re_hdf5::read_attributes(&path, "/observations").unwrap();
    assert_eq!(attrs, vec![("frequency".to_owned(), AttrValue::F64(50.0))]);
}
