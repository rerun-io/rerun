//! Integration tests for `re_parquet`.

// Test helpers intentionally use simplified Arrow constructors; the nuances
// handled by the *_with_metadata / *_with_options variants are irrelevant here.
#![expect(clippy::disallowed_methods)]
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{
    Array as _, FixedSizeListArray, Float32Array, Float64Array, Int64Array, RecordBatch,
    StringArray,
};
use arrow::datatypes::{DataType, Field, Schema};
use itertools::Itertools as _;

use re_chunk::{Chunk, EntityPath};
use re_log_types::TimeType;
use re_parquet::{ColumnGrouping, IndexColumn, IndexType, ParquetConfig, TimeUnit};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn write_parquet_tmp(batch: &RecordBatch) -> std::path::PathBuf {
    use parquet::arrow::ArrowWriter;

    let dir = std::env::temp_dir().join("re_parquet_tests");
    std::fs::create_dir_all(&dir).unwrap();

    let path = dir.join(format!("{}.parquet", re_chunk::ChunkId::new()));
    let file = std::fs::File::create(&path).unwrap();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None).unwrap();
    writer.write(batch).unwrap();
    writer.close().unwrap();

    path
}

fn write_parquet_tmp_with_metadata(
    batch: &RecordBatch,
    kv: Vec<parquet::file::metadata::KeyValue>,
) -> std::path::PathBuf {
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;

    let dir = std::env::temp_dir().join("re_parquet_tests");
    std::fs::create_dir_all(&dir).unwrap();

    let path = dir.join(format!("{}.parquet", re_chunk::ChunkId::new()));
    let file = std::fs::File::create(&path).unwrap();

    let props = WriterProperties::builder()
        .set_key_value_metadata(Some(kv))
        .build();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props)).unwrap();
    writer.write(batch).unwrap();
    writer.close().unwrap();

    path
}

fn load_chunks(path: &std::path::Path, config: &ParquetConfig) -> Vec<Chunk> {
    let prefix = EntityPath::from("/");
    re_parquet::load_parquet(path, config, &prefix)
        .unwrap()
        .try_collect()
        .unwrap()
}

fn data_chunks(chunks: &[Chunk]) -> Vec<&Chunk> {
    chunks
        .iter()
        .filter(|c| c.entity_path() != &EntityPath::properties())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn basic_individual_grouping() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("x", DataType::Float64, false),
            Field::new("y", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
            Arc::new(Float64Array::from(vec![4.0, 5.0, 6.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 2);

    let x_chunk = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/x"))
        .unwrap();
    assert_eq!(x_chunk.num_rows(), 3);
    assert_eq!(x_chunk.num_components(), 1);
    assert!(x_chunk.timelines().contains_key(&"row_index".into()));

    let y_chunk = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/y"))
        .unwrap();
    assert_eq!(y_chunk.num_rows(), 3);
    assert_eq!(y_chunk.num_components(), 1);

    // Prefix-named columns stay separate in individual mode.
    let batch2 = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("camera_rgb", DataType::Float64, false),
            Field::new("camera_depth", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
        ],
    )
    .unwrap();

    let path2 = write_parquet_tmp(&batch2);
    let chunks2 = load_chunks(&path2, &config);
    let data2 = data_chunks(&chunks2);

    assert_eq!(data2.len(), 2);
    assert!(
        data2
            .iter()
            .any(|c| c.entity_path() == &EntityPath::from("/camera_rgb"))
    );
    assert!(
        data2
            .iter()
            .any(|c| c.entity_path() == &EntityPath::from("/camera_depth"))
    );
}

#[test]
fn prefix_grouping() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("camera_rgb", DataType::Float64, false),
            Field::new("camera_depth", DataType::Float64, false),
            Field::new("joint_position", DataType::Float64, false),
            Field::new("joint_velocity", DataType::Float64, false),
            Field::new("action", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1, 2])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
            Arc::new(Float64Array::from(vec![4.0, 5.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.1, 0.2, 0.3])),
            Arc::new(Float64Array::from(vec![0.4, 0.5, 0.6])),
            Arc::new(Float64Array::from(vec![10.0, 20.0, 30.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Prefix {
            delimiter: '_',
            use_structs: true,
        },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // frame_index → timeline, camera_* → 1 struct group, joint_* → 1 struct group, action → 1 single group
    assert_eq!(data.len(), 3);

    // Multi-column prefix groups produce a single struct component named "data"
    let camera = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/camera"))
        .expect("should have /camera entity");
    assert_eq!(camera.num_rows(), 3);
    assert_eq!(
        camera.num_components(),
        1,
        "struct component wraps both columns"
    );
    assert!(camera.timelines().contains_key(&"frame_index".into()));

    // Verify the struct has the expected fields
    let camera_list = camera.components().get_array("data".into()).unwrap();
    let camera_struct = camera_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    assert_eq!(camera_struct.num_columns(), 2);
    assert_eq!(camera_struct.column_by_name("rgb").unwrap().len(), 3);
    assert_eq!(camera_struct.column_by_name("depth").unwrap().len(), 3);

    let joint = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/joint"))
        .expect("should have /joint entity");
    assert_eq!(joint.num_rows(), 3);
    assert_eq!(
        joint.num_components(),
        1,
        "struct component wraps both columns"
    );

    let joint_list = joint.components().get_array("data".into()).unwrap();
    let joint_struct = joint_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    assert_eq!(joint_struct.num_columns(), 2);
    assert!(joint_struct.column_by_name("position").is_some());
    assert!(joint_struct.column_by_name("velocity").is_some());

    // Single-column prefix group: no struct wrapping
    let action = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/action"))
        .expect("should have /action entity");
    assert_eq!(action.num_rows(), 3);
    assert_eq!(action.num_components(), 1);
}

#[test]
fn explicit_timestamp_index() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("ts", DataType::Int64, false),
            Field::new("value", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![100, 200, 300])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "ts".into(),
            index_type: IndexType::Timestamp(TimeUnit::Nanoseconds),
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].entity_path(), &EntityPath::from("/value"));
    assert!(data[0].timelines().contains_key(&"ts".into()));
    let tl = data[0].timelines().get(&"ts".into()).unwrap();
    assert_eq!(tl.timeline().typ(), TimeType::TimestampNs);
}

#[test]
fn explicit_sequence_index() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_id", DataType::Int64, false),
            Field::new("sensor", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1, 2])),
            Arc::new(Float64Array::from(vec![10.0, 20.0, 30.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "frame_id".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].entity_path(), &EntityPath::from("/sensor"));
    assert!(data[0].timelines().contains_key(&"frame_id".into()));
    let tl = data[0].timelines().get(&"frame_id".into()).unwrap();
    assert_eq!(tl.timeline().typ(), TimeType::Sequence);
}

#[test]
fn explicit_duration_index() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("elapsed_us", DataType::Int64, false),
            Field::new("value", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![100, 200, 300])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "elapsed_us".into(),
            index_type: IndexType::Duration(TimeUnit::Microseconds),
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 1);
    let tl = data[0].timelines().get(&"elapsed_us".into()).unwrap();
    assert_eq!(tl.timeline().typ(), TimeType::DurationNs);
    let times: Vec<i64> = tl.times_raw().to_vec();
    assert_eq!(times, vec![100_000, 200_000, 300_000]);
}

#[test]
fn time_unit_scaling() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("ts_ms", DataType::Int64, false),
            Field::new("value", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "ts_ms".into(),
            index_type: IndexType::Timestamp(TimeUnit::Milliseconds),
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 1);
    let tl = data[0].timelines().get(&"ts_ms".into()).unwrap();
    let times: Vec<i64> = tl.times_raw().to_vec();
    assert_eq!(times, vec![1_000_000, 2_000_000, 3_000_000]);
}

#[test]
fn missing_index_column_is_error() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("x", DataType::Float64, false)])),
        vec![Arc::new(Float64Array::from(vec![1.0]))],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "nonexistent".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let prefix = EntityPath::from("/");
    assert!(re_parquet::load_parquet(&path, &config, &prefix).is_err());
}

#[test]
fn static_columns() {
    // Uniform static columns -> timeless chunk
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("value", DataType::Float64, false),
            Field::new("suite", DataType::Utf8, false),
            Field::new("agg", DataType::Utf8, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1, 2])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
            Arc::new(StringArray::from(vec!["test_suite"; 3])),
            Arc::new(StringArray::from(vec!["mean"; 3])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        static_columns: vec!["suite".into(), "agg".into()],
    };
    let chunks = load_chunks(&path, &config);
    let all = data_chunks(&chunks);

    let static_chunks: Vec<_> = all.iter().filter(|c| c.is_static()).collect();
    assert_eq!(static_chunks.len(), 1);
    assert_eq!(static_chunks[0].num_rows(), 1);
    assert_eq!(static_chunks[0].num_components(), 2);

    let data_only: Vec<_> = all.iter().filter(|c| !c.is_static()).collect();
    assert_eq!(data_only.len(), 1);
    assert_eq!(data_only[0].entity_path(), &EntityPath::from("/value"));

    // Non-uniform static column -> error
    let bad_batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("x", DataType::Float64, false),
            Field::new("suite", DataType::Utf8, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(StringArray::from(vec!["a", "b"])),
        ],
    )
    .unwrap();

    let bad_path = write_parquet_tmp(&bad_batch);
    let bad_config = ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        static_columns: vec!["suite".into()],
        ..Default::default()
    };
    let prefix = EntityPath::from("/");
    let result: Vec<_> = re_parquet::load_parquet(&bad_path, &bad_config, &prefix)
        .unwrap()
        .collect();

    assert!(
        result.iter().any(|r| r.is_err()),
        "Non-uniform static column should produce an error"
    );
}

#[test]
fn empty_batch() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, false)])),
        vec![Arc::new(Int64Array::from(Vec::<i64>::new()))],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let chunks = load_chunks(&path, &ParquetConfig::default());
    let data = data_chunks(&chunks);
    assert!(data.is_empty());
}

#[test]
fn file_metadata() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new("x", DataType::Int64, false)])),
        vec![Arc::new(Int64Array::from(vec![1]))],
    )
    .unwrap();

    let kv = vec![
        parquet::file::metadata::KeyValue::new("author".to_owned(), Some("test".to_owned())),
        parquet::file::metadata::KeyValue::new("version".to_owned(), Some("1.0".to_owned())),
    ];
    let path = write_parquet_tmp_with_metadata(&batch, kv);
    let chunks = load_chunks(&path, &ParquetConfig::default());

    let props = chunks
        .iter()
        .find(|c| c.entity_path() == &EntityPath::properties())
        .expect("should have a properties chunk");
    assert!(props.is_static());

    assert!(!data_chunks(&chunks).is_empty());
}

#[test]
fn prefix_grouping_flat() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("camera_rgb", DataType::Float64, false),
            Field::new("camera_depth", DataType::Float64, false),
            Field::new("joint_position", DataType::Float64, false),
            Field::new("joint_velocity", DataType::Float64, false),
            Field::new("action", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1, 2])),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
            Arc::new(Float64Array::from(vec![4.0, 5.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.1, 0.2, 0.3])),
            Arc::new(Float64Array::from(vec![0.4, 0.5, 0.6])),
            Arc::new(Float64Array::from(vec![10.0, 20.0, 30.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Prefix {
            delimiter: '_',
            use_structs: false,
        },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // Flat mode: entries grouped by entity path, no struct wrapping
    assert_eq!(
        data.len(),
        3,
        "one chunk per entity path (camera, joint, action)"
    );

    let camera = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/camera"))
        .expect("should have /camera");
    assert_eq!(camera.num_rows(), 3);
    assert_eq!(
        camera.num_components(),
        2,
        "rgb and depth as separate components"
    );
    assert!(camera.timelines().contains_key(&"frame_index".into()));

    let joint = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/joint"))
        .expect("should have /joint");
    assert_eq!(joint.num_rows(), 3);
    assert_eq!(
        joint.num_components(),
        2,
        "position and velocity as separate components"
    );

    let action = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/action"))
        .expect("should have /action");
    assert_eq!(action.num_rows(), 3);
    assert_eq!(action.num_components(), 1);
}

// ---------------------------------------------------------------------------
// Explicit prefix grouping
// ---------------------------------------------------------------------------

#[test]
fn explicit_prefixes_basic() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("fooa", DataType::Float64, false),
            Field::new("foob", DataType::Float64, false),
            Field::new("fooc", DataType::Float64, false),
            Field::new("cata", DataType::Float64, false),
            Field::new("catb", DataType::Float64, false),
            Field::new("catc", DataType::Float64, false),
            Field::new("other", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
            Arc::new(Float64Array::from(vec![5.0, 6.0])),
            Arc::new(Float64Array::from(vec![7.0, 8.0])),
            Arc::new(Float64Array::from(vec![9.0, 10.0])),
            Arc::new(Float64Array::from(vec![11.0, 12.0])),
            Arc::new(Float64Array::from(vec![13.0, 14.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::ExplicitPrefixes {
            prefixes: vec!["cat".into(), "foo".into()],
            use_structs: true,
        },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // foo group (3 columns), cat group (3 columns), "other" individual
    assert_eq!(data.len(), 3);

    let foo = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/foo"))
        .expect("should have /foo");
    assert_eq!(foo.num_rows(), 2);
    // Multi-column group → struct with 3 fields
    let foo_list = foo.components().get_array("data".into()).unwrap();
    let foo_struct = foo_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    assert_eq!(foo_struct.num_columns(), 3);
    assert!(foo_struct.column_by_name("a").is_some());
    assert!(foo_struct.column_by_name("b").is_some());
    assert!(foo_struct.column_by_name("c").is_some());

    let cat = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/cat"))
        .expect("should have /cat");
    assert_eq!(cat.num_rows(), 2);
    let cat_list = cat.components().get_array("data".into()).unwrap();
    let cat_struct = cat_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    assert_eq!(cat_struct.num_columns(), 3);

    let other = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/other"))
        .expect("should have /other for unmatched column");
    assert_eq!(other.num_rows(), 2);
}

#[test]
fn explicit_prefixes_longest_first() {
    // "catalog" prefix should match "catalogfoo" before "cat" does
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("catx", DataType::Float64, false),
            Field::new("catalogfoo", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0])),
            Arc::new(Float64Array::from(vec![2.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::ExplicitPrefixes {
            prefixes: vec!["cat".into(), "catalog".into()],
            use_structs: true,
        },
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // "catx" → prefix "cat", comp "x" (single-column group, no struct)
    // "catalogfoo" → prefix "catalog", comp "foo" (single-column group)
    assert!(
        data.iter()
            .any(|c| c.entity_path() == &EntityPath::from("/cat")),
        "should have /cat"
    );
    assert!(
        data.iter()
            .any(|c| c.entity_path() == &EntityPath::from("/catalog")),
        "should have /catalog"
    );
}

#[test]
fn explicit_prefixes_underscore_stripping() {
    // prefix "cat" on column "cat_foo" should give comp "foo" (leading _ stripped)
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("cat_foo", DataType::Float64, false),
            Field::new("cat_bar", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0])),
            Arc::new(Float64Array::from(vec![2.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::ExplicitPrefixes {
            prefixes: vec!["cat".into()],
            use_structs: true,
        },
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    let cat = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/cat"))
        .expect("should have /cat");
    let cat_list = cat.components().get_array("data".into()).unwrap();
    let cat_struct = cat_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    // Comp names should be "foo" and "bar", not "_foo" and "_bar"
    assert!(
        cat_struct.column_by_name("foo").is_some(),
        "should have field 'foo'"
    );
    assert!(
        cat_struct.column_by_name("bar").is_some(),
        "should have field 'bar'"
    );
}

// ---------------------------------------------------------------------------
// Archetype mapping via lenses
// ---------------------------------------------------------------------------

/// Example of building a `Transform3D` archetype (translation + rotation quaternion) from a pose
/// table using lenses.
#[test]
fn transform3d_from_struct_via_lens() {
    use re_lenses::op::basic::struct_to_fixed_size_list_f32;
    use re_lenses::{ChunkExt as _, Lens};
    use re_lenses_core::Selector;
    use re_sdk_types::archetypes::Transform3D;

    // A pose table: per-row translation (`pos_*`) and rotation quaternion (`quat_*`).
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("A_pos_x", DataType::Float64, false),
            Field::new("A_pos_y", DataType::Float64, false),
            Field::new("A_pos_z", DataType::Float64, false),
            Field::new("A_quat_x", DataType::Float64, false),
            Field::new("A_quat_y", DataType::Float64, false),
            Field::new("A_quat_z", DataType::Float64, false),
            Field::new("A_quat_w", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
            Arc::new(Float64Array::from(vec![5.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![1.0, 1.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Prefix {
            delimiter: '_',
            use_structs: true,
        },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // The reader produced a single `/A` chunk with one `data` struct component.
    let a_chunk = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/A"))
        .expect("should have /A entity");

    // Map the struct fields into a `Transform3D` archetype with a derive lens: read the
    // `pos_*` / `quat_*` fields off the `data` struct and interleave them into the
    // `FixedSizeList<f32>` arrays the components expect.
    //TODO(RR-4935): use selector function when available
    let lens: Lens = Lens::derive("data")
        .to_component(
            Transform3D::descriptor_translation(),
            Selector::parse(".")
                .unwrap()
                .pipe(struct_to_fixed_size_list_f32(["pos_x", "pos_y", "pos_z"])),
        )
        .to_component(
            Transform3D::descriptor_quaternion(),
            Selector::parse(".")
                .unwrap()
                .pipe(struct_to_fixed_size_list_f32([
                    "quat_x", "quat_y", "quat_z", "quat_w",
                ])),
        )
        .build()
        .unwrap();

    let transformed = a_chunk
        .apply_lenses(&[lens], &re_lenses::default_runtime())
        .unwrap();
    let pose = transformed
        .iter()
        .find(|c| {
            c.components()
                .get_array(Transform3D::descriptor_translation().component)
                .is_some()
        })
        .expect("a chunk carrying the Transform3D translation");

    // Same entity, timeline preserved.
    assert_eq!(pose.entity_path(), &EntityPath::from("/A"));
    assert!(pose.timelines().contains_key(&"frame_index".into()));

    // Translation: FixedSizeList(3, Float32), values interleaved row-major from pos_x/y/z.
    let translation = pose
        .components()
        .get_array(Transform3D::descriptor_translation().component)
        .unwrap();
    let translation = translation
        .values()
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .expect("translation should be a FixedSizeList");
    assert_eq!(translation.value_length(), 3);
    assert_eq!(translation.values().data_type(), &DataType::Float32);
    let translation = translation
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .unwrap();
    assert_eq!(
        translation.values().to_vec(),
        vec![1.0, 3.0, 5.0, 2.0, 4.0, 6.0]
    );

    // Quaternion: FixedSizeList(4, Float32).
    let quaternion = pose
        .components()
        .get_array(Transform3D::descriptor_quaternion().component)
        .unwrap();
    let quaternion = quaternion
        .values()
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .expect("quaternion should be a FixedSizeList");
    assert_eq!(quaternion.value_length(), 4);
    assert_eq!(quaternion.values().data_type(), &DataType::Float32);
    let quaternion = quaternion
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .unwrap();
    assert_eq!(
        quaternion.values().to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
    );
}
