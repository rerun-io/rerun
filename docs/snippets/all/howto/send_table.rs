use std::sync::Arc;

use rerun::external::arrow::array::{ArrayRef, RecordBatch, StringArray, UInt64Array};
use rerun::external::arrow::datatypes::{DataType, Field, Schema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // -- CREATE ARROW RECORD BATCH -------------------------------------------

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("name", DataType::Utf8, false),
    ]));

    // Create a UInt64 array
    let id_array = UInt64Array::from(vec![1, 2, 3, 4, 5]);

    // Create a String array
    let name_array = StringArray::from(vec![
        "Alice",
        "Bob",
        "Charlie",
        "Dave",
        "http://www.rerun.io",
    ]);

    // Convert arrays to ArrayRef (trait objects)
    let arrays: Vec<ArrayRef> = vec![
        Arc::new(id_array) as ArrayRef,
        Arc::new(name_array) as ArrayRef,
    ];

    // Create a RecordBatch
    let dataframe = RecordBatch::try_new(schema.clone(), arrays)?;

    // -- CONNECT TO VIEWER AND SEND TABLE  -----------------------------------

    let client = rerun::ViewerClient::default();
    client.send_table("My dataframe", dataframe);

    Ok(())
}
