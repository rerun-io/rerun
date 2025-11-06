use crate::TempPath;
use arrow::array::{RecordBatch, RecordBatchIterator, record_batch};
use arrow::datatypes as arrow_schema;
use arrow::datatypes::Schema;
use std::collections::HashMap;
use std::sync::Arc;

fn create_example_record_batch(base: u8) -> RecordBatch {
    let batch = record_batch!(
        ("boolean_nullable", Boolean, [Some(true), Some(false), None]),
        ("boolean_not_nullable", Boolean, [true, false, true]),
        (
            "int8_nullable",
            Int8,
            [Some(1 + base as i8), None, Some(2 + base as i8)]
        ),
        (
            "int8_not_nullable",
            Int8,
            [3 + base as i8, 4 + base as i8, 5 + base as i8]
        ),
        (
            "int16_nullable",
            Int16,
            [Some(6 + base as i16), None, Some(7 + base as i16)]
        ),
        (
            "int16_not_nullable",
            Int16,
            [8 + base as i16, 9 + base as i16, 10 + base as i16]
        ),
        (
            "int32_nullable",
            Int32,
            [Some(11 + base as i32), None, Some(12 + base as i32)]
        ),
        (
            "int32_not_nullable",
            Int32,
            [13 + base as i32, 14 + base as i32, 15 + base as i32]
        ),
        (
            "int64_nullable",
            Int64,
            [Some(16 + base as i64), None, Some(17 + base as i64)]
        ),
        (
            "int64_not_nullable",
            Int64,
            [18 + base as i64, 19 + base as i64, 20 + base as i64]
        ),
        (
            "uint8_nullable",
            UInt8,
            [Some(21 + base), None, Some(22 + base)]
        ),
        (
            "uint8_not_nullable",
            UInt8,
            [23 + base, 24 + base, 25 + base]
        ),
        (
            "uint16_nullable",
            UInt16,
            [Some(26 + base as u16), None, Some(27 + base as u16)]
        ),
        (
            "uint16_not_nullable",
            UInt16,
            [28 + base as u16, 29 + base as u16, 30 + base as u16]
        ),
        (
            "uint32_nullable",
            UInt32,
            [Some(31 + base as u32), None, Some(32 + base as u32)]
        ),
        (
            "uint32_not_nullable",
            UInt32,
            [33 + base as u32, 34 + base as u32, 35 + base as u32]
        ),
        (
            "uint64_nullable",
            UInt64,
            [Some(36 + base as u64), None, Some(37 + base as u64)]
        ),
        (
            "uint64_not_nullable",
            UInt64,
            [38 + base as u64, 39 + base as u64, 40 + base as u64]
        ),
        (
            "float32_nullable",
            Float32,
            [Some(41.0 + base as f32), None, Some(42.0 + base as f32)]
        ),
        (
            "float32_not_nullable",
            Float32,
            [43.0 + base as f32, 44.0 + base as f32, 45.0 + base as f32]
        ),
        (
            "float64_nullable",
            Float64,
            [Some(46.0 + base as f64), None, Some(47.0 + base as f64)]
        ),
        (
            "float64_not_nullable",
            Float64,
            [48.0 + base as f64, 49. + base as f64, 50.0 + base as f64]
        ),
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
                        "rerun:is_table_index".to_owned(),
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
