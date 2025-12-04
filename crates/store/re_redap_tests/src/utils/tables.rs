use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchIterator, record_batch};
use arrow::datatypes as arrow_schema;
use arrow::datatypes::Schema;

use crate::TempPath;

fn create_example_record_batch(base: i32) -> RecordBatch {
    let batch = record_batch!(
        ("boolean_nullable", Boolean, [Some(true), Some(false), None]),
        ("boolean_not_nullable", Boolean, [true, false, true]),
        ("int8_nullable", Int8, [Some(1), None, Some(2)]),
        ("int8_not_nullable", Int8, [3, 4, 5]),
        ("int16_nullable", Int16, [Some(1), None, Some(2)]),
        ("int16_not_nullable", Int16, [3, 4, 5]),
        (
            "int32_nullable",
            Int32,
            [Some(11 + base), None, Some(12 + base)]
        ),
        ("int32_not_nullable", Int32, [3, 4, 5]),
        ("int64_nullable", Int64, [Some(1), None, Some(2)]),
        (
            "int64_not_nullable",
            Int64,
            [18 + base as i64, 19 + base as i64, 20 + base as i64]
        ),
        ("uint8_nullable", UInt8, [Some(1), None, Some(2)]),
        ("uint8_not_nullable", UInt8, [3, 4, 5]),
        ("uint16_nullable", UInt16, [Some(1), None, Some(2)]),
        ("uint16_not_nullable", UInt16, [3, 4, 5]),
        ("uint32_nullable", UInt32, [Some(1), None, Some(2)]),
        ("uint32_not_nullable", UInt32, [3, 4, 5]),
        ("uint64_nullable", UInt64, [Some(1), None, Some(2)]),
        ("uint64_not_nullable", UInt64, [3, 4, 5]),
        ("float32_nullable", Float32, [Some(6.0), None, Some(7.0)]),
        ("float32_not_nullable", Float32, [8.0, 9.0, 10.0]),
        ("float64_nullable", Float64, [Some(6.0), None, Some(7.0)]),
        ("float64_not_nullable", Float64, [8.0, 9.0, 10.0]),
        ("utf8_nullable", Utf8, [Some("abc"), Some("def"), None]),
        ("utf8_not_nullable", Utf8, ["ghi", "jkl", "mno"]),
        (
            "large_utf8_nullable",
            LargeUtf8,
            [Some("abc"), Some("def"), None]
        ),
        ("large_utf8_not_nullable", LargeUtf8, ["ghi", "jkl", "mno"])
    )
    .expect("Unable to create record batch");

    // Set the indices
    let schema = Schema::new(
        batch
            .schema()
            .fields
            .iter()
            .map(|field| {
                if field.name() == "int32_nullable" || field.name() == "int64_not_nullable" {
                    field.as_ref().clone().with_metadata(HashMap::from([(
                        re_sorbet::metadata::SORBET_IS_TABLE_INDEX.to_owned(),
                        "true".to_owned(),
                    )]))
                } else {
                    field.as_ref().clone()
                }
            })
            .collect::<Vec<_>>(),
    );

    batch
        .with_schema(Arc::new(schema))
        .expect("unable to create record batch")
}

pub async fn create_simple_lance_dataset() -> anyhow::Result<TempPath> {
    let tmp_dir = tempfile::tempdir()?;

    let batches = vec![
        create_example_record_batch(0),
        create_example_record_batch(100),
    ];
    let schema = batches[0].schema();

    let batches = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
    let path_str = tmp_dir
        .path()
        .to_str()
        .expect("Unable to convert path to string");

    let _ = lance::Dataset::write(batches, path_str, None)
        .await
        .expect("Unable to write lance dataset to directory");
    let path = tmp_dir.path().to_owned();

    Ok(TempPath::new(tmp_dir, path))
}
