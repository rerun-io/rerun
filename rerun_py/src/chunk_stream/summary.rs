//! Shared formatter for `ChunkStore`/`LazyStore` summary strings.
//!
//! Used by both `PyChunkStoreInternal::summary` (chunks-based) and
//! `PyLazyStoreInternal::summary` (manifest-based) so the two paths produce
//! identical lines on the same logical chunk.

/// One line of a chunk-store summary.
///
/// `timelines` and `cols` must be pre-sorted by the caller.
pub(super) struct SummaryRow {
    pub entity_path: String,
    pub num_rows: u64,
    pub is_static: bool,
    pub timelines: Vec<String>,
    pub cols: Vec<String>,
}

/// Format chunk-store rows into a deterministic snapshot string.
///
/// One line per row, sorted by `(entity_path, !is_static)`. Format:
/// `{entity_path} rows={n} static={bool} timelines=[…] cols=[…]`.
pub(super) fn format_summary(rows: impl IntoIterator<Item = SummaryRow>) -> String {
    let mut rows: Vec<SummaryRow> = rows.into_iter().collect();
    rows.sort_by(|a, b| {
        a.entity_path
            .cmp(&b.entity_path)
            .then_with(|| a.is_static.cmp(&b.is_static).reverse())
    });

    let mut lines = Vec::with_capacity(rows.len());
    for row in &rows {
        let timelines_str = row
            .timelines
            .iter()
            .map(|t| format!("'{t}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let cols_str = row
            .cols
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let is_static = if row.is_static { "True" } else { "False" };
        lines.push(format!(
            "{entity_path} rows={rows} static={is_static} timelines=[{timelines_str}] cols=[{cols_str}]",
            entity_path = row.entity_path,
            rows = row.num_rows,
        ));
    }

    lines.join("\n")
}
