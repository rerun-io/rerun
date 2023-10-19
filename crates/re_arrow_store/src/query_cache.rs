use std::collections::{BTreeMap, VecDeque};

use re_log_types::{DataCell, DataRow, DataTable, EntityPath, RowId, TimeInt, Timeline};
use re_types_core::{ComponentBatch, ComponentName};

use crate::{LatestAtQuery, RangeQuery, WriteResult};

// ---

pub struct QueryCache {
    timelines: BTreeMap<Query, TimelineCache>,
}

#[derive(Clone)]
enum Query {
    LatestAt(LatestAtQuery),
    Range(RangeQuery),
}

struct TimelineCache {
    times: VecDeque<TimeInt>,
    values: VecDeque<Box<dyn ComponentBatch>>,
}

// TODO: gotta invalidate on the write path.

impl QueryCache {
    pub fn insert_table(&mut self, table: &DataTable) -> WriteResult<()> {
        for row in table.to_rows() {
            self.insert_row(&row?)?;
        }

        Ok(())
    }

    pub fn insert_row(&mut self, row: &DataRow) -> WriteResult<()> {
        todo!()
    }
}

impl QueryCache {
    pub fn latest_at<const N: usize>(
        &self,
        query: &LatestAtQuery,
        ent_path: &EntityPath,
        primary: ComponentName,
        components: &[ComponentName; N],
    ) -> Option<(RowId, [Option<&dyn ComponentBatch>; N])> {
        todo!()
    }

    // pub fn range<'a, const N: usize>(
    //     &'a self,
    //     query: &RangeQuery,
    //     ent_path: &EntityPath,
    //     components: [ComponentName; N],
    // ) -> impl Iterator<Item = (Option<TimeInt>, RowId, [Option<&dyn ComponentBatch>; N])> + 'a {
    //     todo!()
    // }
}

// TODO: by far the hardest problem is going to be invalidation
//
// TODO: how far does keeping deserialized data gets us?
