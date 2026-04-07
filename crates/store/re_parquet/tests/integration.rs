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
    ColumnGrouping, ComponentRule, IndexColumn, IndexType, MappedComponent, ParquetConfig, TimeUnit,
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
        column_grouping: ColumnGrouping::Prefix { delimiter: '_' },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    // frame_index -> timeline, camera_* -> 1 group, joint_* -> 1 group, action -> 1 group
    assert_eq!(data.len(), 3);

    let camera = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/camera"))
        .expect("should have /camera entity");
    assert_eq!(camera.num_rows(), 3);
    assert_eq!(camera.num_components(), 2);
    assert!(camera.timelines().contains_key(&"frame_index".into()));

    let joint = data
        .iter()
        .find(|c| c.entity_path() == &EntityPath::from("/joint"))
        .expect("should have /joint entity");
    assert_eq!(joint.num_rows(), 3);
    assert_eq!(joint.num_components(), 2);

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
        column_grouping: ColumnGrouping::Prefix { delimiter: '_' },
        index_columns: vec![IndexColumn {
            name: "frame_index".into(),
            index_type: IndexType::Sequence,
        }],
        archetype_rules: vec![
            ComponentRule {
                suffixes: vec!["_pos_x".into(), "_pos_y".into(), "_pos_z".into()],
                target: MappedComponent::Translation3D,
            },
            ComponentRule {
                suffixes: vec![
                    "_quat_x".into(),
                    "_quat_y".into(),
                    "_quat_z".into(),
                    "_quat_w".into(),
                ],
                target: MappedComponent::RotationQuat,
            },
        ],
        ..Default::default()
    };
    let chunks = load_chunks(&path, &config);
    let data = data_chunks(&chunks);

    let a_chunks: Vec<_> = data
        .iter()
        .filter(|c| c.entity_path() == &EntityPath::from("/A"))
        .collect();

    assert_eq!(a_chunks.len(), 2);

    // One chunk should have 2 components (translation + quaternion)
    let archetype_chunk = a_chunks
        .iter()
        .find(|c| c.num_components() == 2)
        .expect("should have archetype chunk with 2 components");
    assert_eq!(archetype_chunk.num_rows(), 2);
    assert!(
        archetype_chunk
            .timelines()
            .contains_key(&"frame_index".into())
    );

    // The other chunk should have 1 component (speed)
    let leftover_chunk = a_chunks
        .iter()
        .find(|c| c.num_components() == 1)
        .expect("should have leftover chunk with 1 component");
    assert_eq!(leftover_chunk.num_rows(), 2);
}
