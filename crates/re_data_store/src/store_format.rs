use arrow2::datatypes::Metadata;
use re_format::{format_bytes, format_uint};
use re_log_types::TimeInt;
use re_types_core::SizeBytes as _;

use crate::{DataStore, IndexedBucket, IndexedTable, StaticTable};

// --- Data store ---

impl std::fmt::Display for DataStore {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            config,
            type_registry: _,
            metadata_registry: _,
            tables,
            static_tables,
            insert_id: _,
            query_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        f.write_str("DataStore {\n")?;

        f.write_str(&indent::indent_all_by(4, format!("id: {id}\n")))?;
        f.write_str(&indent::indent_all_by(4, format!("config: {config:?}\n")))?;

        {
            f.write_str(&indent::indent_all_by(
                4,
                format!(
                    "{} static tables, for a total of {}\n",
                    static_tables.len(),
                    format_bytes(self.static_size_bytes() as _),
                ),
            ))?;
            f.write_str(&indent::indent_all_by(4, "static_tables: [\n"))?;
            for static_table in static_tables.values() {
                f.write_str(&indent::indent_all_by(8, "StaticTable {\n"))?;
                f.write_str(&indent::indent_all_by(12, static_table.to_string() + "\n"))?;
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
                    format_uint(self.num_temporal_rows())
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
            entity_path,
            buckets,
            all_components: _,
            buckets_num_rows: _,
            buckets_size_bytes: _,
        } = self;

        f.write_fmt(format_args!("timeline: {}\n", timeline.name()))?;
        f.write_fmt(format_args!("entity: {entity_path}\n"))?;

        f.write_fmt(format_args!(
            "size: {} buckets for a total of {} across {} total rows\n",
            self.buckets.len(),
            format_bytes(self.total_size_bytes() as _),
            format_uint(self.num_rows()),
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
            format_uint(self.num_rows()),
        ))?;

        let time_range = {
            let time_range = &self.inner.read().time_range;
            if time_range.min() != TimeInt::MAX && time_range.max() != TimeInt::MIN {
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
        re_format_arrow::format_dataframe(
            Metadata::default(),
            &schema.fields,
            columns.columns().iter().map(|array| &**array),
        )
        .fmt(f)?;

        writeln!(f)
    }
}

// --- Static ---

impl std::fmt::Display for StaticTable {
    #[allow(clippy::string_add)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("entity: {}\n", self.entity_path))?;

        f.write_fmt(format_args!(
            "size: {} across {} cells\n",
            format_bytes(
                self.cells
                    .values()
                    .map(|cell| cell.cell.total_size_bytes())
                    .sum::<u64>() as _
            ),
            format_uint(self.cells.len()),
        ))?;

        let (schema, columns) = self.serialize().map_err(|err| {
            re_log::error_once!("couldn't display static table: {err}");
            std::fmt::Error
        })?;
        re_format_arrow::format_dataframe(
            Metadata::default(),
            &schema.fields,
            columns.columns().iter().map(|array| &**array),
        )
        .fmt(f)?;

        writeln!(f)
    }
}
