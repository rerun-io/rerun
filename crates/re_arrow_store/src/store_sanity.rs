use re_log_types::{
    ComponentName, DataCellColumn, COLUMN_NUM_INSTANCES, COLUMN_ROW_ID, COLUMN_TIMEPOINT,
};

use crate::{DataStore, IndexedBucket, IndexedBucketInner, IndexedTable, PersistentIndexedTable};

// ---

#[derive(thiserror::Error, Debug)]
pub enum SanityError {
    #[error("Column '{component}' has too few/many rows: got {got} instead of {expected}")]
    ColumnLengthMismatch {
        component: ComponentName,
        expected: u64,
        got: u64,
    },

    #[error("Couldn't find any column for the configured cluster key ('{cluster_key}')")]
    ClusterColumnMissing { cluster_key: ComponentName },

    #[error("The cluster column must be dense, found holes: {cluster_column:?}")]
    ClusterColumnSparse { cluster_column: Box<DataCellColumn> },

    #[error("Found overlapping indexed buckets: {t1_max_formatted} ({t1_max}) <-> {t2_max_formatted} ({t2_max})")]
    OverlappingBuckets {
        t1_max: i64,
        t1_max_formatted: String,
        t2_max: i64,
        t2_max_formatted: String,
    },
}

pub type SanityResult<T> = ::std::result::Result<T, SanityError>;

// --- Data store ---

impl DataStore {
    /// Runs the sanity check suite for the entire datastore.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        crate::profile_function!();

        for table in self.timeless_tables.values() {
            table.sanity_check()?;
        }

        for table in self.tables.values() {
            table.sanity_check()?;
        }

        Ok(())
    }
}

// --- Persistent Indices ---

impl PersistentIndexedTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        crate::profile_function!();

        let Self {
            ent_path: _,
            cluster_key,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            total_size_bytes: _, // TODO(#1619)
        } = self;

        // All columns should be `Self::num_rows` long.
        {
            let num_rows = self.total_rows();

            let column_lengths = [
                (!col_insert_id.is_empty())
                    .then(|| (DataStore::insert_id_key(), col_insert_id.len())), //
                Some((COLUMN_ROW_ID.into(), col_row_id.len())),
                Some((COLUMN_NUM_INSTANCES.into(), col_num_instances.len())),
            ]
            .into_iter()
            .flatten()
            .chain(
                columns
                    .iter()
                    .map(|(component, column)| (*component, column.len())),
            )
            .map(|(component, len)| (component, len as u64));

            for (component, len) in column_lengths {
                if len != num_rows {
                    return Err(SanityError::ColumnLengthMismatch {
                        component,
                        expected: num_rows,
                        got: len,
                    });
                }
            }
        }

        // The cluster column must be fully dense.
        {
            let cluster_column =
                columns
                    .get(cluster_key)
                    .ok_or(SanityError::ClusterColumnMissing {
                        cluster_key: *cluster_key,
                    })?;
            if !cluster_column.iter().all(|cell| cell.is_some()) {
                return Err(SanityError::ClusterColumnSparse {
                    cluster_column: cluster_column.clone().into(),
                });
            }
        }

        // TODO(#1619): recomputing shouldnt change the size

        Ok(())
    }
}

// --- Indices ---

impl IndexedTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        crate::profile_function!();

        // No two buckets should ever overlap time-range-wise.
        {
            let time_ranges = self
                .buckets
                .values()
                .map(|bucket| bucket.inner.read().time_range)
                .collect::<Vec<_>>();
            for time_ranges in time_ranges.windows(2) {
                let &[t1, t2] = time_ranges else { unreachable!() };
                if t1.max.as_i64() >= t2.min.as_i64() {
                    return Err(SanityError::OverlappingBuckets {
                        t1_max: t1.max.as_i64(),
                        t1_max_formatted: self.timeline.typ().format(t1.max),
                        t2_max: t2.max.as_i64(),
                        t2_max_formatted: self.timeline.typ().format(t2.max),
                    });
                }
            }
        }

        // Run individual bucket sanity check suites too.
        for bucket in self.buckets.values() {
            bucket.sanity_check()?;
        }

        Ok(())
    }
}

impl IndexedBucket {
    /// Runs the sanity check suite for the entire bucket.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        crate::profile_function!();

        let Self {
            timeline: _,
            cluster_key,
            inner,
        } = self;

        let IndexedBucketInner {
            is_sorted: _,
            time_range: _,
            col_time,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
            total_size_bytes: _, // TODO(#1619)
        } = &*inner.read();

        // All columns should be `Self::num_rows` long.
        {
            let num_rows = self.total_rows();

            let column_lengths = [
                (!col_insert_id.is_empty())
                    .then(|| (DataStore::insert_id_key(), col_insert_id.len())), //
                Some((COLUMN_TIMEPOINT.into(), col_time.len())),
                Some((COLUMN_ROW_ID.into(), col_row_id.len())),
                Some((COLUMN_NUM_INSTANCES.into(), col_num_instances.len())),
            ]
            .into_iter()
            .flatten()
            .chain(
                columns
                    .iter()
                    .map(|(component, column)| (*component, column.len())),
            )
            .map(|(component, len)| (component, len as u64));

            for (component, len) in column_lengths {
                if len != num_rows {
                    return Err(SanityError::ColumnLengthMismatch {
                        component,
                        expected: num_rows,
                        got: len,
                    });
                }
            }
        }

        // The cluster column must be fully dense.
        {
            let cluster_column =
                columns
                    .get(cluster_key)
                    .ok_or(SanityError::ClusterColumnMissing {
                        cluster_key: *cluster_key,
                    })?;
            if !cluster_column.iter().all(|cell| cell.is_some()) {
                return Err(SanityError::ClusterColumnSparse {
                    cluster_column: cluster_column.clone().into(),
                });
            }
        }

        // TODO(#1619): recomputing shouldnt change the size

        Ok(())
    }
}
