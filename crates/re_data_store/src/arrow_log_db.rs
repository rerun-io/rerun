use std::{
    collections::{btree_map::Entry, BTreeMap},
    io::Cursor,
};

use polars::export::arrow::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};
use re_log_types::{ArrowMsg, ObjPath};

//use arrow2::{
//    array::Array,
//    io::ipc::read::{read_stream_metadata, StreamReader, StreamState},
//};

pub struct ArrowLogDb {
    objs: BTreeMap<ObjPath, polars::frame::DataFrame>,
}

fn stream_reader(data: &[u8]) -> StreamReader<Cursor<&[u8]>> {
    let mut cursor = Cursor::new(data);
    let metadata = read_stream_metadata(&mut cursor).unwrap();
    StreamReader::new(cursor, metadata, None)
}

impl ArrowLogDb {
    pub fn new() -> Self {
        Self {
            objs: BTreeMap::new(),
        }
    }
    pub fn add_msg(&mut self, msg: ArrowMsg) {
        let stream = stream_reader(&msg.data);
        let schema = stream.metadata().schema.clone();

        let name = schema.metadata.get("ARROW:extension:name");

        let path = schema
            .metadata
            .get("ARROW:extension:metadata")
            .map(|path| ObjPath::from(path.as_str()))
            .expect("Bad ObjPath");

        for item in stream {
            if let StreamState::Some(chunk) = item.unwrap() {
                let df =
                    polars::frame::DataFrame::try_from((chunk, schema.fields.as_slice())).unwrap();

                match self.objs.entry(path.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(df);
                    }
                    Entry::Occupied(mut e) => {
                        e.get_mut().extend(&df);
                    }
                }
            }
        }

        //re_log::info!("Got ArrowMsg: {:?}", chunk);
        for (path, df) in self.objs.iter() {
            println!("{path}: {df:?}");

            let x = df.explode(&["rect", "rgbacolor"]);
            dbg!(x);
        }
    }
}
