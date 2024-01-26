use re_format::{format_bytes, format_number};
use re_log_types::TimeInt;
use re_types_core::SizeBytes as _;

use crate::{IndexedBucket, IndexedTable, PersistentIndexedTable, UnaryDataStore};

// --- Data store ---

impl std::fmt::Display for UnaryDataStore {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            cluster_key,
            config,
            cluster_cell_cache: _,
            type_registry: _,
            metadata_registry: _,
            tables,
            timeless_tables,
            insert_id: _,
            query_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        f.write_str("DataStore {\n")?;

        f.write_str(&indent::indent_all_by(4, format!("id: {id}\n")))?;
        f.write_str(&indent::indent_all_by(
            4,
            format!("cluster_key: {cluster_key:?}\n"),
        ))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} timeless indexed tables, for a total of {} across {} total rows\n",
                    timeless_tables.len(),
                    format_bytes(self.timeless_size_bytes() as _),
                    format_number(self.num_timeless_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "timeless_tables: [\n"))?;
            for table in timeless_tables.values() {
                f.write_str(&indent::indent_all_by(8, "PersistentIndexedTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} indexed tables, for a total of {} across {} total rows\n",
                    tables.len(),
                    format_bytes(self.temporal_size_bytes() as _),
                    format_number(self.num_temporal_rows() as _)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "tables: [\n"))?;
            for table in tables.values() {
                f.write_str(&indent::indent_all_by(8, "IndexedTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, table.to_string() + "\n"))?;
                f.write_str(&indent::indent_all_by(8, "}\n"))?;
            }
            f.write_str(&indent::indent_all_by(4, "]\n"))?;
        }

        f.write_str("}")?;

        Ok(())
    }
}

// --- Temporal ---

impl std::fmt::Display for IndexedTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            timeline,
            ent_path,
            buckets,
            cluster_key: _,
            all_components: _,
            buckets_num_rows: _,
            buckets_size_bytes: _,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {ent_path}\n"))?;

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_number(self.num_rows() as _),
        ))?;
        f.write_str("buckets: [\n")?;
        for (time, bucket) in buckets {
            f.write_str(&indent::indent_all_by(4, "IndexedBucket {\n"))?;
            f.write_str(&indent::indent_all_by(
                8,
                format!(
                    "index time bound: >= {}\n",
                    timeline.typ().format_utc(*time)
                ),
            ))?;
            f.write_str(&indent::indent_all_by(8, bucket.to_string()))?;
            f.write_str(&indent::indent_all_by(4, "}\n"))?;
        }
        f.write_str("]")?;

        Ok(())
    }
}

impl std::fmt::Display for IndexedBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.num_rows() as _),
        ))?;

        let time_range = {
            let time_range = &self.inner.read().time_range;
            if time_range.min != TimeInt::MAX && time_range.max != TimeInt::MIN {
                format!(
                    "    - {}: {}",
                    self.timeline.name(),
                    self.timeline.format_time_range_utc(time_range)
                )
            } else {
                "time range: N/A\n".to_owned()
            }
        };
        f.write_fmt(format_args!("{time_range}\n"))?;

        let (schema, columns) = self.serialize().map_err(|err| {
            re_log::error_once!("couldn't display indexed bucket: {err}");
            std::fmt::Error
        })?;
        re_format::arrow::format_table(
            columns.columns(),
            schema.fields.iter().map(|field| field.name.as_str()),
        )
        .fmt(f)?;

        writeln!(f)
    }
}

// --- Timeless ---

impl std::fmt::Display for PersistentIndexedTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("entity: {}\n", self.ent_path))?;

        f.write_fmt(format_args!(
            "size: {} across {} rows\n",
            format_bytes(self.total_size_bytes() as _),
            format_number(self.inner.read().num_rows() as _),
        ))?;

        let (schema, columns) = self.serialize().map_err(|err| {
            re_log::error_once!("couldn't display timeless indexed table: {err}");
            std::fmt::Error
        })?;
        re_format::arrow::format_table(
            columns.columns(),
            schema.fields.iter().map(|field| field.name.as_str()),
        )
        .fmt(f)?;

        writeln!(f)
    }
}
