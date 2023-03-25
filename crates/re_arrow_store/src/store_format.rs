use arrow2::array::UInt64Array;
use re_format::{arrow, format_bytes, format_number};

use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexRowNr, IndexTable,
    PersistentComponentTable, PersistentIndexTable, RowIndex, RowIndexKind,
};

// --- Indices & offsets ---

impl std::fmt::Display for RowIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind() {
            RowIndexKind::Temporal => f.write_fmt(format_args!("Temporal({})", self.0)),
            RowIndexKind::Timeless => f.write_fmt(format_args!("Timeless({})", self.0)),
        }
    }
}

impl std::fmt::Display for IndexRowNr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

// --- Data store ---

impl std::fmt::Display for DataStore {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            cluster_key,
            config,
            cluster_comp_cache: _,
            messages: _,
            indices,
            components,
            timeless_indices,
            timeless_components,
            insert_id: _,
            query_id: _,
            gc_id: _,
        } = self;

        f.write_str("DataStore {\n")?;

        f.write_str(&indent::indent_all_by(
            4,
            format!("cluster_key: {cluster_key:?}\n"),
        ))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} timeless index tables, for a total of {} across {} total rows\n",
                    timeless_indices.len(),
                    format_bytes(self.total_timeless_index_size_bytes() as _),
                    format_number(self.total_timeless_index_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "timeless_indices: [\n"))?;
            for table in timeless_indices.values() {
                f.write_str(&indent::indent_all_by(8, "PersistentIndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }
        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} persistent component tables, for a total of {} across {} total rows\n",
                    timeless_components.len(),
                    format_bytes(self.total_timeless_component_size_bytes() as _),
                    format_number(self.total_timeless_component_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "timeless_components: [\n"))?;
            for table in timeless_components.values() {
                f.write_str(&indent::indent_all_by(8, "PersistentComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} index tables, for a total of {} across {} total rows\n",
                    indices.len(),
                    format_bytes(self.total_temporal_index_size_bytes() as _),
                    format_number(self.total_temporal_index_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "indices: [\n"))?;
            for table in indices.values() {
                f.write_str(&indent::indent_all_by(8, "IndexTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }
        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} component tables, for a total of {} across {} total rows\n",
                    components.len(),
                    format_bytes(self.total_temporal_component_size_bytes() as _),
                    format_number(self.total_temporal_component_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "components: [\n"))?;
            for table in components.values() {
                f.write_str(&indent::indent_all_by(8, "ComponentTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        f.write_str("}")?;

        Ok(())
    }
}

// --- Persistent Indices ---

impl std::fmt::Display for PersistentIndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            ent_path,
            cluster_key: _,
            num_rows: _,
            indices: _,
            all_components: _,
        } = self;

        f.write_fmt(format_args!("entity: {ent_path}\n"))?;

        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        let (col_names, cols): (Vec<_>, Vec<_>) = {
            self.indices
                .iter()
                .map(|(name, index)| {
                    (
                        name.to_string(),
                        UInt64Array::from(
                            index
                                .iter()
                                .map(|row_idx| row_idx.map(|row_idx| row_idx.as_u64()))
                                .collect::<Vec<_>>(),
                        ),
                    )
                })
                .unzip()
        };

        let values = cols.into_iter().map(|c| c.boxed());
        let table = arrow::format_table(values, col_names);

        f.write_fmt(format_args!("data:\n{table}\n"))?;

        Ok(())
    }
}

// --- Indices ---

impl std::fmt::Display for IndexTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
            cluster_key: _,
            all_components: _,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {ent_path}\n"))?;

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;
        f.write_str("buckets: [\n")?;
        for (time, bucket) in buckets.iter() {
            f.write_str(&indent::indent_all_by(4, "IndexBucket {\n"))?;
            f.write_str(&indent::indent_all_by(
                8,
                format!("index time bound: >= {}\n", timeline.typ().format(*time),),
            ))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string()))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl std::fmt::Display for IndexBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        let time_range = {
            let time_range = &self.indices.read().time_range;
            if time_range.min.as_i64() != i64::MAX && time_range.max.as_i64() != i64::MIN {
                self.timeline.format_time_range(time_range)
            } else {
                "time range: N/A\n".to_owned()
            }
        };
        f.write_fmt(format_args!("{time_range}\n"))?;

        let (timeline_name, times) = self.times();
        let (col_names, cols): (Vec<_>, Vec<_>) = {
            self.indices
                .read()
                .indices
                .iter()
                .map(|(name, index)| {
                    (
                        name.to_string(),
                        UInt64Array::from(
                            index
                                .iter()
                                .map(|row_idx| row_idx.map(|row_idx| row_idx.as_u64()))
                                .collect::<Vec<_>>(),
                        ),
                    )
                })
                .unzip()
        };

        let names = std::iter::once(timeline_name).chain(col_names);
        let values = std::iter::once(times.boxed()).chain(cols.into_iter().map(|c| c.boxed()));
        let table = arrow::format_table(values, names);

        let is_sorted = self.is_sorted();
        f.write_fmt(format_args!("data (sorted={is_sorted}):\n{table}\n"))?;

        Ok(())
    }
}

// --- Persistent Components ---

impl std::fmt::Display for PersistentComponentTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            chunks,
            total_rows,
            total_size_bytes,
        } = self;

        f.write_fmt(format_args!("name: {name}\n"))?;
        if matches!(
            std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS").as_deref(),
            Ok("1")
        ) {
            f.write_fmt(format_args!("datatype: {datatype:#?}\n"))?;
        }

        f.write_fmt(format_args!(
            "size: {} across {} total rows\n",
            format_bytes(*total_size_bytes as _),
            format_number(*total_rows as _),
        ))?;

        let data = {
            use arrow2::compute::concatenate::concatenate;
            let chunks = chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            concatenate(&chunks).unwrap()
        };

        let table = arrow::format_table([data], [self.name.as_str()]);
        f.write_fmt(format_args!("{table}\n"))?;

        Ok(())
    }
}

// --- Components ---

impl std::fmt::Display for ComponentTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            datatype,
            buckets,
        } = self;

        f.write_fmt(format_args!("name: {name}\n"))?;
        if matches!(
            std::env::var("RERUN_DATA_STORE_DISPLAY_SCHEMAS").as_deref(),
            Ok("1")
        ) {
            f.write_fmt(format_args!("datatype: {datatype:#?}\n"))?;
        }

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;
        f.write_str("buckets: [\n")?;
        for bucket in buckets {
            f.write_str(&indent::indent_all_by(4, "ComponentBucket {\n"))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string()))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl std::fmt::Display for ComponentBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.total_rows() as _),
        ))?;

        f.write_fmt(format_args!(
            "row range: from {} to {} (all inclusive)\n",
            self.row_offset,
            // Component buckets can never be empty at the moment:
            // - the first bucket is always initialized with a single empty row
            // - all buckets that follow are lazily instantiated when data get inserted
            //
            // TODO(#439): is that still true with deletion?
            // TODO(#589): support for non-unit-length chunks
            self.row_offset
                + self
                    .chunks
                    .len()
                    .checked_sub(1)
                    .expect("buckets are never empty") as u64,
        ))?;

        f.write_fmt(format_args!("archived: {}\n", self.archived))?;
        f.write_str("time ranges:\n")?;
        for (timeline, time_range) in &self.time_ranges {
            f.write_fmt(format_args!(
                "{}\n",
                &timeline.format_time_range(time_range)
            ))?;
        }

        let data = {
            use arrow2::compute::concatenate::concatenate;
            let chunks = self.chunks.iter().map(|chunk| &**chunk).collect::<Vec<_>>();
            concatenate(&chunks).unwrap()
        };

        let table = arrow::format_table([data], [self.name.as_str()]);
        f.write_fmt(format_args!("{table}\n"))?;

        Ok(())
    }
}
