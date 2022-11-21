use std::{
    borrow::{self, Borrow},
    collections::{btree_map::Entry, BTreeMap},
    io::Cursor,
};

use polars::export::arrow::io::ipc::read::{read_stream_metadata, StreamReader, StreamState};
use polars::prelude::*;
use re_log_types::{ArrowMsg, ObjPath};

pub struct LogDb {
    objs: BTreeMap<ObjPath, polars::frame::DataFrame>,
}

fn stream_reader(data: &[u8]) -> StreamReader<Cursor<&[u8]>> {
    let mut cursor = Cursor::new(data);
    let metadata = read_stream_metadata(&mut cursor).unwrap();
    StreamReader::new(cursor, metadata, None)
}

impl LogDb {
    pub fn new() -> Self {
        Self {
            objs: BTreeMap::new(),
        }
    }

    pub fn push_new_chunk(&mut self, obj_path: &ObjPath, schema: &ArrowSchema, chunk: ArrowChunk) {
        let df = polars::frame::DataFrame::try_from((chunk, schema.fields.as_slice())).unwrap();

        match self.objs.entry(obj_path.clone()) {
            Entry::Vacant(e) => {
                e.insert(df);
            }
            Entry::Occupied(mut e) => {
                let left = e.get_mut();
                crate::append_unified(e.get_mut(), &df).unwrap();
            }
        }
    }

    pub fn consume_msg(&mut self, msg: ArrowMsg) {
        let stream = stream_reader(&msg.data);
        let arrow_schema = stream.metadata().schema.clone();

        let name = arrow_schema.metadata.get("ARROW:extension:name");
        let obj_path = arrow_schema
            .metadata
            .get("ARROW:extension:metadata")
            .map(|path| ObjPath::from(path.as_str()))
            .expect("Bad ObjPath");

        for item in stream {
            if let StreamState::Some(chunk) = item.unwrap() {
                self.push_new_chunk(&obj_path, &arrow_schema, chunk);
            }
        }

        //re_log::info!("Got ArrowMsg: {:?}", chunk);
        for (path, df) in self.objs.iter() {
            println!("{path}: {df:?}");

            //let x = df.explode(&["rect", "rgbacolor"]);
            //dbg!(x);
        }
    }
}
