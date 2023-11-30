use re_log_types::{DataCellColumn, NumInstances, RowId, TimeRange};
use re_types_core::{ComponentName, Loggable, SizeBytes as _};

use crate::{DataStore, IndexedBucket, IndexedBucketInner, IndexedTable, PersistentIndexedTable};

// ---

/// Returned by the `sanity_check` family of function when an invariant violation has been detected
/// in the `DataStore`'s internal datastructures.
/// These violations can only stem from a bug in the store's implementation itself.
#[derive(thiserror::Error, Debug)]
pub enum SanityError {
    #[error(
        "Reported time range for indexed bucket is out of sync: got {got:?}, expected {expected:?}"
    )]
    TimeRangeOutOfSync { expected: TimeRange, got: TimeRange },

    #[error("Reported size for {origin} is out of sync: got {got}, expected {expected}")]
    SizeOutOfSync {
        origin: &'static str,
        expected: String,
        got: String,
    },

    #[error("Reported number of rows for {origin} is out of sync: got {got}, expected {expected}")]
    RowsOutOfSync {
        origin: &'static str,
        expected: String,
        got: String,
    },

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
        re_tracing::profile_function!();

        for table in self.timeless_tables.values() {
            table.sanity_check()?;
        }

        for table in self.tables.values() {
            table.sanity_check()?;
        }

        Ok(())
    }
}

// --- Temporal ---

impl IndexedTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        re_tracing::profile_function!();

        // No two buckets should ever overlap time-range-wise.
        {
            let time_ranges = self
                .buckets
                .values()
                .map(|bucket| bucket.inner.read().time_range)
                .collect::<Vec<_>>();
            for time_ranges in time_ranges.windows(2) {
                let &[t1, t2] = time_ranges else {
                    unreachable!()
                };
                if t1.max.as_i64() >= t2.min.as_i64() {
                    return Err(SanityError::OverlappingBuckets {
                        t1_max: t1.max.as_i64(),
                        t1_max_formatted: self.timeline.typ().format_utc(t1.max),
                        t2_max: t2.max.as_i64(),
                        t2_max_formatted: self.timeline.typ().format_utc(t2.max),
                    });
                }
            }
        }

        // Make sure row numbers aren't out of sync
        {
            let num_rows = self.num_rows();
            let num_rows_uncached = self.num_rows_uncached();
            if num_rows != num_rows_uncached {
                return Err(SanityError::RowsOutOfSync {
                    origin: std::any::type_name::<Self>(),
                    expected: re_format::format_number(num_rows_uncached as _),
                    got: re_format::format_number(num_rows as _),
                });
            }
        }

        // Run individual bucket sanity check suites too.
        for bucket in self.buckets.values() {
            bucket.sanity_check()?;
        }

        // Make sure size values aren't out of sync
        {
            let total_size_bytes = self.total_size_bytes();
            let total_size_bytes_uncached = self.size_bytes_uncached();
            if total_size_bytes != total_size_bytes_uncached {
                return Err(SanityError::SizeOutOfSync {
                    origin: std::any::type_name::<Self>(),
                    expected: re_format::format_bytes(total_size_bytes_uncached as _),
                    got: re_format::format_bytes(total_size_bytes as _),
                });
            }
        }

        Ok(())
    }
}

impl IndexedBucket {
    /// Runs the sanity check suite for the entire bucket.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        re_tracing::profile_function!();

        let Self {
            id: _,
            timeline: _,
            cluster_key,
            inner,
        } = self;

        {
            let IndexedBucketInner {
                is_sorted: _,
                time_range,
                col_time,
                col_insert_id,
                col_row_id,
                newest_row_id,
                col_num_instances,
                columns,
                size_bytes: _,
            } = &*inner.read();

            // Time ranges are eagerly maintained.
            {
                let mut times = col_time.clone();
                let (times, &mut []) = times.as_mut_slices() else {
                    unreachable!(); // TODO
                };
                times.sort();

                let expected_min = times.first().copied().unwrap_or(i64::MAX).into();
                let expected_max = times.last().copied().unwrap_or(i64::MIN).into();
                let expected_time_range = TimeRange::new(expected_min, expected_max);

                if expected_time_range != *time_range {
                    return Err(SanityError::TimeRangeOutOfSync {
                        expected: expected_time_range,
                        got: *time_range,
                    });
                }
            }

            // TODO
            // // Make sure `newest_row_id` isn't out of sync
            // {
            //     let expected = col_row_id
            //         .iter()
            //         .copied()
            //         .reduce(RowId::max)
            //         .unwrap_or(RowId::ZERO);
            //     assert_eq!(expected, *newest_row_id); // TODO: actual error
            // }

            // All columns should be `Self::num_rows` long.
            {
                const COLUMN_TIMEPOINT: &str = "rerun.controls.TimePoint";

                let num_rows = self.num_rows();

                let column_lengths = [
                    (!col_insert_id.is_empty())
                        .then(|| (DataStore::insert_id_component_name(), col_insert_id.len())), //
                    Some((COLUMN_TIMEPOINT.into(), col_time.len())),
                    Some((RowId::name(), col_row_id.len())),
                    Some((NumInstances::name(), col_num_instances.len())),
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
            if self.num_rows() > 0 {
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
        }

        // Make sure size values aren't out of sync
        {
            let size_bytes = inner.read().size_bytes;
            let size_bytes_uncached = inner.write().compute_size_bytes();
            if size_bytes != size_bytes_uncached {
                return Err(SanityError::SizeOutOfSync {
                    origin: std::any::type_name::<Self>(),
                    expected: re_format::format_bytes(size_bytes_uncached as _),
                    got: re_format::format_bytes(size_bytes as _),
                });
            }
        }

        Ok(())
    }
}

// --- Timeless ---

impl PersistentIndexedTable {
    /// Runs the sanity check suite for the entire table.
    ///
    /// Returns an error if anything looks wrong.
    pub fn sanity_check(&self) -> SanityResult<()> {
        re_tracing::profile_function!();

        let Self {
            ent_path: _,
            cluster_key,
            col_insert_id,
            col_row_id,
            col_num_instances,
            columns,
        } = self;

        // All columns should be `Self::num_rows` long.
        {
            let num_rows = self.num_rows();

            let column_lengths = [
                (!col_insert_id.is_empty())
                    .then(|| (DataStore::insert_id_component_name(), col_insert_id.len())), //
                Some((RowId::name(), col_row_id.len())),
                Some((NumInstances::name(), col_num_instances.len())),
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
        if self.num_rows() > 0 {
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

        Ok(())
    }
}
