//! Column grouping algorithm for mapping parquet columns to Rerun entities.

use re_chunk::EntityPath;

use crate::config::ColumnGrouping;

/// A single source column inside a [`ColumnGroup`].
pub(crate) struct ColumnEntry {
    /// Index of the source column in the record batch.
    pub col_idx: usize,

    /// Component name the column is emitted under.
    pub comp_name: String,
}

/// A set of columns that will be emitted as a single chunk.
pub(crate) struct ColumnGroup {
    pub entity_path: EntityPath,
    pub entries: Vec<ColumnEntry>,
}

/// Build a [`ColumnGroup`] from a base entity path and its `(col_idx, comp_name)` columns.
fn column_group(entity_path: EntityPath, columns: Vec<(usize, String)>) -> ColumnGroup {
    ColumnGroup {
        entity_path,
        entries: columns
            .into_iter()
            .map(|(col_idx, comp_name)| ColumnEntry { col_idx, comp_name })
            .collect(),
    }
}

/// Compute column groups by splitting/joining column names according to `grouping`.
///
/// Each group carries its raw columns; whether a multi-column group is wrapped into a
/// single `Struct` component or emitted flat is decided downstream in `streaming.rs`
/// based on `use_structs`.
pub(crate) fn compute_column_groups(
    schema: &arrow::datatypes::Schema,
    excluded: &std::collections::HashSet<usize>,
    entity_path_prefix: &EntityPath,
    grouping: &ColumnGrouping,
) -> Vec<ColumnGroup> {
    match grouping {
        ColumnGrouping::Individual => schema
            .fields()
            .iter()
            .enumerate()
            .filter(|(i, _)| !excluded.contains(i))
            .map(|(i, field)| {
                column_group(
                    entity_path_prefix.join(&EntityPath::from(field.name().as_str())),
                    vec![(i, field.name().clone())],
                )
            })
            .collect(),

        ColumnGrouping::Prefix {
            delimiter,
            use_structs: _,
        } => {
            let mut prefix_groups: std::collections::BTreeMap<String, Vec<(usize, String)>> =
                std::collections::BTreeMap::new();

            for (i, field) in schema.fields().iter().enumerate() {
                if excluded.contains(&i) {
                    continue;
                }
                let name = field.name().as_str();
                let (prefix, comp_name) = match name.find(*delimiter) {
                    Some(pos) if pos + delimiter.len_utf8() < name.len() => {
                        (&name[..pos], &name[pos + delimiter.len_utf8()..])
                    }
                    _ => (name, name),
                };
                prefix_groups
                    .entry(prefix.to_owned())
                    .or_default()
                    .push((i, comp_name.to_owned()));
            }

            prefix_groups
                .into_iter()
                .map(|(prefix, comp_entries)| {
                    column_group(
                        entity_path_prefix.join(&EntityPath::from(prefix.as_str())),
                        comp_entries,
                    )
                })
                .collect()
        }

        ColumnGrouping::ExplicitPrefixes {
            prefixes,
            use_structs: _,
        } => {
            // Sort prefixes longest-first so "catalog" is tried before "cat".
            let mut sorted_prefixes = prefixes.clone();
            sorted_prefixes.sort_by_key(|b| std::cmp::Reverse(b.len()));

            let mut prefix_groups: std::collections::BTreeMap<String, Vec<(usize, String)>> =
                std::collections::BTreeMap::new();
            let mut unmatched: Vec<(usize, String)> = Vec::new();

            for (i, field) in schema.fields().iter().enumerate() {
                if excluded.contains(&i) {
                    continue;
                }
                let name = field.name().as_str();
                let mut matched = false;
                for prefix in &sorted_prefixes {
                    if let Some(remainder) = name.strip_prefix(prefix.as_str()) {
                        if remainder.is_empty() {
                            // Exact match (column name == prefix): treat as individual.
                            break;
                        }
                        // Strip one leading underscore so prefix "cat" on "cat_foo" → "foo".
                        let comp_name = remainder.strip_prefix('_').unwrap_or(remainder);
                        prefix_groups
                            .entry(prefix.clone())
                            .or_default()
                            .push((i, comp_name.to_owned()));
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    unmatched.push((i, name.to_owned()));
                }
            }

            let mut groups: Vec<ColumnGroup> = prefix_groups
                .into_iter()
                .map(|(prefix, comp_entries)| {
                    column_group(
                        entity_path_prefix.join(&EntityPath::from(prefix.as_str())),
                        comp_entries,
                    )
                })
                .collect();

            // Unmatched columns: each gets its own individual group.
            for (i, name) in unmatched {
                let entity_path = entity_path_prefix.join(&EntityPath::from(name.as_str()));
                groups.push(column_group(entity_path, vec![(i, name)]));
            }

            groups
        }
    }
}
