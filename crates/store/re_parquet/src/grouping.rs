//! Column grouping algorithm for mapping parquet columns to Rerun entities.

use re_chunk::EntityPath;

use crate::config::{ColumnGrouping, ComponentRule, MappedComponent, ScalarSuffixGroup};

/// An entry inside a [`ColumnGroup`].
pub(crate) enum ColumnGroupEntry {
    /// A raw column, emitted as-is with `wrap_in_fixed_size_list`.
    Raw { col_idx: usize, comp_name: String },

    /// Multiple columns combined into a typed archetype component.
    Archetype {
        col_indices: Vec<usize>,
        target: MappedComponent,
    },

    /// Multiple columns combined into N-instance `Scalars` with named series.
    ScalarGroup {
        col_indices: Vec<usize>,
        names: Vec<String>,
    },
}

/// A set of columns that will be emitted as a single chunk.
pub(crate) struct ColumnGroup {
    pub entity_path: EntityPath,
    pub entries: Vec<ColumnGroupEntry>,
}

/// Compute column groups: prefix-split first, then apply archetype suffix rules within each group.
pub(crate) fn compute_column_groups(
    schema: &arrow::datatypes::Schema,
    excluded: &std::collections::HashSet<usize>,
    entity_path_prefix: &EntityPath,
    grouping: &ColumnGrouping,
    archetype_rules: &[ComponentRule],
    scalar_suffixes: &[ScalarSuffixGroup],
) -> Vec<ColumnGroup> {
    match grouping {
        ColumnGrouping::Individual => {
            let (mut groups, consumed) =
                match_archetype_rules_raw(schema, excluded, entity_path_prefix, archetype_rules);
            for (i, field) in schema.fields().iter().enumerate() {
                if excluded.contains(&i) || consumed.contains(&i) {
                    continue;
                }
                groups.push(ColumnGroup {
                    entity_path: entity_path_prefix.join(&EntityPath::from(field.name().as_str())),
                    entries: vec![ColumnGroupEntry::Raw {
                        col_idx: i,
                        comp_name: field.name().clone(),
                    }],
                });
            }
            groups
        }

        ColumnGrouping::Prefix { delimiter } => {
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

            let mut groups = Vec::new();
            for (prefix, comp_entries) in prefix_groups {
                let base_path = entity_path_prefix.join(&EntityPath::from(prefix.as_str()));

                let (arch_sub_groups, raw_entries) =
                    match_suffix_rules_in_group(&comp_entries, archetype_rules, scalar_suffixes);

                for (sub_prefix, entries) in arch_sub_groups {
                    let entity_path = if sub_prefix.is_empty() {
                        base_path.clone()
                    } else {
                        base_path.join(&EntityPath::from(sub_prefix.as_str()))
                    };
                    groups.push(ColumnGroup {
                        entity_path,
                        entries,
                    });
                }

                if !raw_entries.is_empty() {
                    groups.push(ColumnGroup {
                        entity_path: base_path,
                        entries: raw_entries,
                    });
                }
            }
            groups
        }
    }
}

/// Apply archetype and scalar suffix rules within a prefix group.
fn match_suffix_rules_in_group(
    entries: &[(usize, String)],
    archetype_rules: &[ComponentRule],
    scalar_rules: &[ScalarSuffixGroup],
) -> (
    std::collections::BTreeMap<String, Vec<ColumnGroupEntry>>,
    Vec<ColumnGroupEntry>,
) {
    let mut consumed = std::collections::HashSet::new();
    let mut sub_groups: std::collections::BTreeMap<String, Vec<ColumnGroupEntry>> =
        std::collections::BTreeMap::new();

    let name_to_idx: std::collections::HashMap<&str, usize> = entries
        .iter()
        .map(|(idx, name)| (name.as_str(), *idx))
        .collect();

    let try_match_suffixes = |suffixes: &[String],
                              consumed: &mut std::collections::HashSet<usize>|
     -> Vec<(String, Vec<usize>)> {
        if suffixes.is_empty() {
            return vec![];
        }

        let stripped: Vec<&str> = suffixes
            .iter()
            .map(|s| s.strip_prefix('_').unwrap_or(s.as_str()))
            .collect();
        let first = stripped[0];

        let mut matches = vec![];
        for &(idx, ref comp_name) in entries {
            if consumed.contains(&idx) {
                continue;
            }
            let Some(raw_sub) = comp_name.strip_suffix(first) else {
                continue;
            };

            let mut col_indices = vec![idx];
            let mut all_found = true;

            for &suffix in &stripped[1..] {
                let expected = format!("{raw_sub}{suffix}");
                match name_to_idx.get(expected.as_str()) {
                    Some(&other_idx) if !consumed.contains(&other_idx) => {
                        col_indices.push(other_idx);
                    }
                    _ => {
                        all_found = false;
                        break;
                    }
                }
            }

            if all_found {
                for &ci in &col_indices {
                    consumed.insert(ci);
                }
                let sub_prefix = raw_sub.strip_suffix('_').unwrap_or(raw_sub).to_owned();
                matches.push((sub_prefix, col_indices));
            }
        }
        matches
    };

    for rule in archetype_rules {
        for (sub_prefix, col_indices) in try_match_suffixes(&rule.suffixes, &mut consumed) {
            sub_groups
                .entry(sub_prefix)
                .or_default()
                .push(ColumnGroupEntry::Archetype {
                    col_indices,
                    target: rule.target,
                });
        }
    }

    for rule in scalar_rules {
        for (sub_prefix, col_indices) in try_match_suffixes(&rule.suffixes, &mut consumed) {
            sub_groups
                .entry(sub_prefix)
                .or_default()
                .push(ColumnGroupEntry::ScalarGroup {
                    col_indices,
                    names: rule.names.clone(),
                });
        }
    }

    let raw = entries
        .iter()
        .filter(|(idx, _)| !consumed.contains(idx))
        .map(|(idx, name)| ColumnGroupEntry::Raw {
            col_idx: *idx,
            comp_name: name.clone(),
        })
        .collect();

    (sub_groups, raw)
}

/// Scan raw column names for suffix-pattern matches (used by [`ColumnGrouping::Individual`]).
fn match_archetype_rules_raw(
    schema: &arrow::datatypes::Schema,
    excluded: &std::collections::HashSet<usize>,
    entity_path_prefix: &EntityPath,
    rules: &[ComponentRule],
) -> (Vec<ColumnGroup>, std::collections::HashSet<usize>) {
    let mut consumed = std::collections::HashSet::new();
    let mut archetype_entries: std::collections::BTreeMap<String, Vec<ColumnGroupEntry>> =
        std::collections::BTreeMap::new();

    let name_to_idx: std::collections::HashMap<&str, usize> = schema
        .fields()
        .iter()
        .enumerate()
        .filter(|(i, _)| !excluded.contains(i))
        .map(|(i, f)| (f.name().as_str(), i))
        .collect();

    for rule in rules {
        if rule.suffixes.is_empty() {
            continue;
        }
        let first_suffix = &rule.suffixes[0];

        for (&name, &idx) in &name_to_idx {
            if consumed.contains(&idx) {
                continue;
            }
            let Some(prefix) = name.strip_suffix(first_suffix.as_str()) else {
                continue;
            };

            let mut col_indices = vec![idx];
            let mut all_found = true;

            for suffix in &rule.suffixes[1..] {
                let expected = format!("{prefix}{suffix}");
                match name_to_idx.get(expected.as_str()) {
                    Some(&other_idx) if !consumed.contains(&other_idx) => {
                        col_indices.push(other_idx);
                    }
                    _ => {
                        all_found = false;
                        break;
                    }
                }
            }

            if all_found {
                for &ci in &col_indices {
                    consumed.insert(ci);
                }
                archetype_entries
                    .entry(prefix.to_owned())
                    .or_default()
                    .push(ColumnGroupEntry::Archetype {
                        col_indices,
                        target: rule.target,
                    });
            }
        }
    }

    let groups = archetype_entries
        .into_iter()
        .map(|(prefix, entries)| {
            let entity_path = if prefix.is_empty() {
                entity_path_prefix.clone()
            } else {
                entity_path_prefix.join(&EntityPath::from(prefix.as_str()))
            };
            ColumnGroup {
                entity_path,
                entries,
            }
        })
        .collect();

    (groups, consumed)
}
