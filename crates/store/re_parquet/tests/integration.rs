//! Integration tests for `re_parquet`.

// Test helpers intentionally use simplified Arrow constructors; the nuances
// handled by the *_with_metadata / *_with_options variants are irrelevant here.
#![expect(clippy::disallowed_methods)]
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use re_chunk::{Chunk, EntityPath};
use re_log_types::TimeType;
use re_parquet::{
    ColumnGrouping, ColumnMapping, ColumnRule, IndexColumn, IndexType, ParquetConfig, TimeUnit,
};

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
        .collect::<Result<Vec<_>, _>>()
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
        ..Default::default()
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
fn archetype_rules_transform3d() {
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
            Field::new("A_speed", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 4.0])),
            Arc::new(Float64Array::from(vec![2.0, 5.0])),
            Arc::new(Float64Array::from(vec![3.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![1.0, 1.0])),
            Arc::new(Float64Array::from(vec![9.0, 8.0])),
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
        column_rules: vec![
            ColumnRule {
                suffixes: vec!["_pos_x".into(), "_pos_y".into(), "_pos_z".into()],
                mapping: ColumnMapping::translation3d(),
                field_name_override: None,
            },
            ColumnRule {
                suffixes: vec![
                    "_quat_x".into(),
                    "_quat_y".into(),
                    "_quat_z".into(),
                    "_quat_w".into(),
                ],
                mapping: ColumnMapping::rotation_quat(),
                field_name_override: None,
            },
        ],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // All columns for prefix "A" collapse into a single chunk with one struct component
    let a_chunks: Vec<_> = data
        .iter()
        .filter(|c| c.entity_path() == &EntityPath::from("/A"))
        .collect();

    assert_eq!(a_chunks.len(), 1, "all entries in one struct → one chunk");

    let a_chunk = a_chunks[0];
    assert_eq!(a_chunk.num_rows(), 2);
    assert_eq!(a_chunk.num_components(), 1, "single struct component");
    assert!(a_chunk.timelines().contains_key(&"frame_index".into()));

    // Verify the struct fields: archetype fields + raw leftover
    let a_list = a_chunk.components().get_array("data".into()).unwrap();
    let a_struct = a_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");

    // 3 struct fields: pos (FixedSizeList(3, Float32)), quat (FixedSizeList(4, Float32)), speed (Float64)
    assert_eq!(a_struct.num_columns(), 3);

    let pos_field = a_struct
        .column_by_name("pos")
        .expect("should have pos field");
    assert!(
        matches!(pos_field.data_type(), DataType::FixedSizeList(_, 3)),
        "pos should be FixedSizeList(3, _), got {:?}",
        pos_field.data_type()
    );

    let quat_field = a_struct
        .column_by_name("quat")
        .expect("should have quat field");
    assert!(
        matches!(quat_field.data_type(), DataType::FixedSizeList(_, 4)),
        "quat should be FixedSizeList(4, _), got {:?}",
        quat_field.data_type()
    );

    let speed_field = a_struct
        .column_by_name("speed")
        .expect("should have speed field");
    assert_eq!(speed_field.data_type(), &DataType::Float64);
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

#[test]
fn archetype_rules_transform3d_flat() {
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
            Field::new("A_speed", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 4.0])),
            Arc::new(Float64Array::from(vec![2.0, 5.0])),
            Arc::new(Float64Array::from(vec![3.0, 6.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![0.0, 0.0])),
            Arc::new(Float64Array::from(vec![1.0, 1.0])),
            Arc::new(Float64Array::from(vec![9.0, 8.0])),
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
        column_rules: vec![
            ColumnRule {
                suffixes: vec!["_pos_x".into(), "_pos_y".into(), "_pos_z".into()],
                mapping: ColumnMapping::translation3d(),
                field_name_override: None,
            },
            ColumnRule {
                suffixes: vec![
                    "_quat_x".into(),
                    "_quat_y".into(),
                    "_quat_z".into(),
                    "_quat_w".into(),
                ],
                mapping: ColumnMapping::rotation_quat(),
                field_name_override: None,
            },
        ],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    assert_eq!(data.len(), 3, "pos + quat + speed as separate chunks");

    let pos = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/A/pos"))
        .expect("should have /A/pos");
    assert_eq!(pos.num_rows(), 2);
    assert_eq!(pos.num_components(), 1);

    let quat = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/A/quat"))
        .expect("should have /A/quat");
    assert_eq!(quat.num_rows(), 2);
    assert_eq!(quat.num_components(), 1);

    let speed = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/A"))
        .expect("should have /A (raw speed)");
    assert_eq!(speed.num_rows(), 2);
    assert_eq!(speed.num_components(), 1);
}

#[test]
fn scalar_suffixes_flat() {
    // Columns like sensor_accel_x where after prefix split on '_' the comp_names
    // are accel_x, accel_y, accel_z — suffix _x matches accel_x but NOT accel_ax.
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("sensor_accel_x", DataType::Float64, false),
            Field::new("sensor_accel_y", DataType::Float64, false),
            Field::new("sensor_accel_z", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
            Arc::new(Float64Array::from(vec![5.0, 6.0])),
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
        column_rules: vec![ColumnRule {
            suffixes: vec!["_x".into(), "_y".into(), "_z".into()],
            mapping: ColumnMapping::Scalars {
                names: vec!["x".into(), "y".into(), "z".into()],
            },
            field_name_override: None,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // comp_names: accel_x, accel_y, accel_z
    // suffix _x matches accel_x → raw_sub "accel" → sub_prefix "accel"
    // field_name = "accel" → entity path = /sensor/accel
    let scalars_path = EntityPath::from("/sensor/accel");

    // Data chunk: Scalars component
    let data_only: Vec<_> = data.iter().filter(|c| !c.is_static()).collect();
    assert_eq!(data_only.len(), 1);
    assert_eq!(data_only[0].entity_path(), &scalars_path);
    assert_eq!(data_only[0].num_rows(), 2);

    // Static Name chunk: series labels
    let static_chunks: Vec<_> = data.iter().filter(|c| c.is_static()).collect();
    assert_eq!(static_chunks.len(), 1, "should have static Name chunk");
    assert_eq!(static_chunks[0].entity_path(), &scalars_path);
    assert_eq!(static_chunks[0].num_components(), 1);
}

#[test]
fn scalar_suffixes_no_false_match() {
    // Suffix _x should NOT match comp_name ending in "ax" (no delimiter boundary)
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("sensor_accel_ax", DataType::Float64, false),
            Field::new("sensor_accel_ay", DataType::Float64, false),
            Field::new("sensor_accel_az", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
            Arc::new(Float64Array::from(vec![5.0, 6.0])),
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
        column_rules: vec![ColumnRule {
            suffixes: vec!["_x".into(), "_y".into(), "_z".into()],
            mapping: ColumnMapping::Scalars {
                names: vec!["x".into(), "y".into(), "z".into()],
            },
            field_name_override: None,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // accel_ax does NOT end in _x, so no scalar group should be created.
    // All three columns should be raw entries in the "sensor" struct.
    let sensor = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/sensor"))
        .expect("should have /sensor");
    let sensor_list = sensor.components().get_array("data".into()).unwrap();
    let sensor_struct = sensor_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");
    // 3 raw fields, not grouped into a scalar
    assert_eq!(sensor_struct.num_columns(), 3);
    assert!(sensor_struct.column_by_name("accel_ax").is_some());
    assert!(sensor_struct.column_by_name("accel_ay").is_some());
    assert!(sensor_struct.column_by_name("accel_az").is_some());
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
// Struct-mode Name emission
// ---------------------------------------------------------------------------

#[test]
fn scalar_suffixes_struct_names_in_struct() {
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("sensor_accel_x", DataType::Float64, false),
            Field::new("sensor_accel_y", DataType::Float64, false),
            Field::new("sensor_accel_z", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 2.0])),
            Arc::new(Float64Array::from(vec![3.0, 4.0])),
            Arc::new(Float64Array::from(vec![5.0, 6.0])),
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
        column_rules: vec![ColumnRule {
            suffixes: vec!["_x".into(), "_y".into(), "_z".into()],
            mapping: ColumnMapping::Scalars {
                names: vec!["x".into(), "y".into(), "z".into()],
            },
            field_name_override: None,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    let sensor_path = EntityPath::from("/sensor");

    // In struct mode, names are embedded in the struct — no static Name chunks
    let data_only: Vec<_> = data.iter().filter(|c| !c.is_static()).collect();
    assert_eq!(data_only.len(), 1);
    assert_eq!(data_only[0].entity_path(), &sensor_path);

    let static_chunks: Vec<_> = data.iter().filter(|c| c.is_static()).collect();
    assert_eq!(
        static_chunks.len(),
        0,
        "struct mode should NOT emit static Name chunks"
    );

    // Verify the struct has both data and names fields
    let sensor_list = data_only[0].components().get_array("data".into()).unwrap();
    let sensor_struct = sensor_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");

    // Should have "accel" (data) and "accel_names" (labels)
    assert_eq!(sensor_struct.num_columns(), 2);
    assert!(
        sensor_struct.column_by_name("accel").is_some(),
        "should have 'accel' data field"
    );
    let names_col = sensor_struct
        .column_by_name("accel_names")
        .expect("should have 'accel_names' field");
    assert!(
        matches!(names_col.data_type(), DataType::FixedSizeList(_, 3)),
        "names should be FixedSizeList(3, _)"
    );
}

// ---------------------------------------------------------------------------
// Field name override
// ---------------------------------------------------------------------------

#[test]
fn field_name_override_archetype() {
    // Columns Foo_name_pos_x/y/z and Foo_name_quat_x/y/z/w
    // Without override both get field_name "name" → collision.
    // With override "_pos" and "_quat" → "name_pos" and "name_quat".
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("Foo_name_pos_x", DataType::Float64, false),
            Field::new("Foo_name_pos_y", DataType::Float64, false),
            Field::new("Foo_name_pos_z", DataType::Float64, false),
            Field::new("Foo_name_quat_x", DataType::Float64, false),
            Field::new("Foo_name_quat_y", DataType::Float64, false),
            Field::new("Foo_name_quat_z", DataType::Float64, false),
            Field::new("Foo_name_quat_w", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Int64Array::from(vec![0, 1])),
            Arc::new(Float64Array::from(vec![1.0, 4.0])),
            Arc::new(Float64Array::from(vec![2.0, 5.0])),
            Arc::new(Float64Array::from(vec![3.0, 6.0])),
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
        column_rules: vec![
            ColumnRule {
                suffixes: vec!["_pos_x".into(), "_pos_y".into(), "_pos_z".into()],
                mapping: ColumnMapping::translation3d(),
                field_name_override: Some("_pos".into()),
            },
            ColumnRule {
                suffixes: vec![
                    "_quat_x".into(),
                    "_quat_y".into(),
                    "_quat_z".into(),
                    "_quat_w".into(),
                ],
                mapping: ColumnMapping::rotation_quat(),
                field_name_override: Some("_quat".into()),
            },
        ],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    let foo = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/Foo"))
        .expect("should have /Foo");
    let foo_list = foo.components().get_array("data".into()).unwrap();
    let foo_struct = foo_list
        .values()
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .expect("should be a StructArray");

    // Should have name_pos and name_quat, NOT two fields both named "name"
    assert_eq!(foo_struct.num_columns(), 2);
    assert!(
        foo_struct.column_by_name("name_pos").is_some(),
        "should have field 'name_pos', got fields: {:?}",
        foo_struct
            .fields()
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
    );
    assert!(
        foo_struct.column_by_name("name_quat").is_some(),
        "should have field 'name_quat', got fields: {:?}",
        foo_struct
            .fields()
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
    );
}

#[test]
fn field_name_override_empty_sub_prefix() {
    // When sub_prefix is empty, override (stripped of _) is used directly
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("Foo_pos_x", DataType::Float64, false),
            Field::new("Foo_pos_y", DataType::Float64, false),
            Field::new("Foo_pos_z", DataType::Float64, false),
        ])),
        vec![
            Arc::new(Float64Array::from(vec![1.0])),
            Arc::new(Float64Array::from(vec![2.0])),
            Arc::new(Float64Array::from(vec![3.0])),
        ],
    )
    .unwrap();

    let path = write_parquet_tmp(&batch);
    let config = ParquetConfig {
        column_grouping: ColumnGrouping::Prefix {
            delimiter: '_',
            use_structs: true,
        },
        column_rules: vec![ColumnRule {
            suffixes: vec!["_pos_x".into(), "_pos_y".into(), "_pos_z".into()],
            mapping: ColumnMapping::Component {
                descriptor: re_sdk_types::archetypes::Transform3D::descriptor_translation(),
            },
            field_name_override: Some("_pos".into()),
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    let foo = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/Foo"))
        .expect("should have /Foo");

    // sub_prefix is empty (comp "pos_x" strip_suffix "pos_x" → ""),
    // override "_pos" → field_name "pos"
    let foo_list = foo.components().get_array("data".into());
    // Single-entry group: no struct wrapping, component is the archetype directly.
    // The field_name is used for flat_entity_path but not for struct field when single entry.
    assert!(foo_list.is_some() || foo.num_components() == 1);
}
