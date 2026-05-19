//! Column grouping algorithm for mapping parquet columns to Rerun entities.

use re_chunk::EntityPath;
use re_sdk_types::ComponentDescriptor;

use crate::config::{ColumnGrouping, ColumnMapping, ColumnRule};

/// An entry inside a [`ColumnGroup`].
pub(crate) enum ColumnGroupEntry {
    /// A raw column, emitted as-is with `wrap_in_fixed_size_list`.
    Raw { col_idx: usize, comp_name: String },

    /// Multiple columns combined into a typed archetype component.
    Component {
        col_indices: Vec<usize>,
        descriptor: ComponentDescriptor,

        /// Struct field name when this entry is part of a multi-entry prefix group.
        field_name: String,
    },

    /// Multiple columns combined into N-instance `Scalars` with named series.
    ScalarGroup {
        col_indices: Vec<usize>,
        names: Vec<String>,

        /// Struct field name when this entry is part of a multi-entry prefix group.
        field_name: String,
    },

    /// Translation + rotation columns combined into a `Transform3D`.
    ///
    /// In struct mode, emitted as a nested struct with `translation` and
    /// `quaternion` fields. In flat mode, emitted as two separate components.
    Transform {
        translation_col_indices: Vec<usize>,
        rotation_col_indices: Vec<usize>,
        translation_descriptor: ComponentDescriptor,
        rotation_descriptor: ComponentDescriptor,

        /// Struct field name when this entry is part of a multi-entry prefix group.
        field_name: String,
    },
}

/// A set of columns that will be emitted as a single chunk.
pub(crate) struct ColumnGroup {
    pub entity_path: EntityPath,
    pub entries: Vec<ColumnGroupEntry>,
}

/// Compute column groups: prefix-split first, then apply column rules within each group.
pub(crate) fn compute_column_groups(
    schema: &arrow::datatypes::Schema,
    excluded: &std::collections::HashSet<usize>,
    entity_path_prefix: &EntityPath,
    grouping: &ColumnGrouping,
    column_rules: &[ColumnRule],
) -> Vec<ColumnGroup> {
    warn_shadowed_rules(column_rules);

    match grouping {
        ColumnGrouping::Individual => {
            let (mut groups, consumed) =
                match_rules_raw(schema, excluded, entity_path_prefix, column_rules);
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

            let mut groups = Vec::new();
            for (prefix, comp_entries) in prefix_groups {
                let base_path = entity_path_prefix.join(&EntityPath::from(prefix.as_str()));
                let all_entries = match_rules_in_group(&comp_entries, column_rules);
                if !all_entries.is_empty() {
                    groups.push(ColumnGroup {
                        entity_path: base_path,
                        entries: all_entries,
                    });
                }
            }
            groups
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

            let mut groups = Vec::new();

            for (prefix, comp_entries) in prefix_groups {
                let base_path = entity_path_prefix.join(&EntityPath::from(prefix.as_str()));
                let all_entries = match_rules_in_group(&comp_entries, column_rules);
                if !all_entries.is_empty() {
                    groups.push(ColumnGroup {
                        entity_path: base_path,
                        entries: all_entries,
                    });
                }
            }

            // Unmatched columns: each gets its own individual group.
            for (i, name) in unmatched {
                groups.push(ColumnGroup {
                    entity_path: entity_path_prefix.join(&EntityPath::from(name.as_str())),
                    entries: vec![ColumnGroupEntry::Raw {
                        col_idx: i,
                        comp_name: name,
                    }],
                });
            }

            groups
        }
    }
}

/// Apply column rules within a prefix group.
///
/// Returns a flat list of all entries (component + scalar + raw).
fn match_rules_in_group(
    entries: &[(usize, String)],
    column_rules: &[ColumnRule],
) -> Vec<ColumnGroupEntry> {
    let mut consumed = std::collections::HashSet::new();
    let mut all_entries = Vec::new();

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

        // Strip the leading `_` from suffixes (it corresponds to the delimiter
        // already consumed by prefix splitting), but require that `raw_sub`
        // is either empty or ends with `_`. This enforces that the suffix
        // matched at an underscore boundary within the comp_name.
        //
        // Example: suffix `_x`, stripped to `x`.
        //   - comp_name `accel_x` → raw_sub `accel_` → ends with `_` ✓
        //   - comp_name `accel_ax` → raw_sub `accel_a` → NOT ending with `_` ✗
        //   - comp_name `x` → raw_sub `` → empty ✓ (matched at start)
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

            // Enforce underscore boundary: raw_sub must be empty or end with '_'.
            if !raw_sub.is_empty() && !raw_sub.ends_with('_') {
                continue;
            }

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

    for rule in column_rules {
        match &rule.mapping {
            ColumnMapping::Transform { rotation_suffixes } => {
                // Match translation and rotation suffix sets independently,
                // then join on sub_prefix to form Transform entries.
                use re_sdk_types::archetypes::Transform3D;

                let translation_matches = try_match_suffixes(&rule.suffixes, &mut consumed);
                let rotation_matches = try_match_suffixes(rotation_suffixes, &mut consumed);

                // Index rotation matches by sub_prefix for joining.
                let mut rot_by_prefix: std::collections::HashMap<String, Vec<usize>> =
                    std::collections::HashMap::new();
                for (sub_prefix, col_indices) in &rotation_matches {
                    rot_by_prefix.insert(sub_prefix.clone(), col_indices.clone());
                }

                let mut unmatched_translations: Vec<(String, Vec<usize>)> = Vec::new();
                for (sub_prefix, trans_indices) in translation_matches {
                    if let Some(rot_indices) = rot_by_prefix.remove(&sub_prefix) {
                        let field_name = derive_field_name(
                            &sub_prefix,
                            &suffix_common_prefix(&rule.suffixes),
                            rule.field_name_override.as_deref(),
                        );
                        all_entries.push(ColumnGroupEntry::Transform {
                            translation_col_indices: trans_indices,
                            rotation_col_indices: rot_indices,
                            translation_descriptor: Transform3D::descriptor_translation(),
                            rotation_descriptor: Transform3D::descriptor_quaternion(),
                            field_name,
                        });
                    } else {
                        unmatched_translations.push((sub_prefix, trans_indices));
                    }
                }

                // Unconsume columns from unmatched translation/rotation sets
                // so they can be picked up by later rules.
                for (_prefix, indices) in &unmatched_translations {
                    for &ci in indices {
                        consumed.remove(&ci);
                    }
                }
                for indices in rot_by_prefix.values() {
                    for &ci in indices {
                        consumed.remove(&ci);
                    }
                }
            }
            mapping => {
                let suffix_fallback = suffix_common_prefix(&rule.suffixes);
                for (sub_prefix, col_indices) in try_match_suffixes(&rule.suffixes, &mut consumed) {
                    let field_name = derive_field_name(
                        &sub_prefix,
                        &suffix_fallback,
                        rule.field_name_override.as_deref(),
                    );
                    match mapping {
                        ColumnMapping::Component { descriptor } => {
                            all_entries.push(ColumnGroupEntry::Component {
                                col_indices,
                                descriptor: descriptor.clone(),
                                field_name,
                            });
                        }
                        ColumnMapping::Scalars { names } => {
                            let mut field_name = field_name;
                            if field_name.is_empty() {
                                field_name = "scalars".to_owned();
                            }
                            all_entries.push(ColumnGroupEntry::ScalarGroup {
                                col_indices,
                                names: names.clone(),
                                field_name,
                            });
                        }
                        ColumnMapping::Transform { .. } => unreachable!(),
                    }
                }
            }
        }
    }

    all_entries.extend(
        entries
            .iter()
            .filter(|(idx, _)| !consumed.contains(idx))
            .map(|(idx, name)| ColumnGroupEntry::Raw {
                col_idx: *idx,
                comp_name: name.clone(),
            }),
    );

    all_entries
}

/// Scan raw column names for suffix-pattern matches (used by [`ColumnGrouping::Individual`]).
fn match_rules_raw(
    schema: &arrow::datatypes::Schema,
    excluded: &std::collections::HashSet<usize>,
    entity_path_prefix: &EntityPath,
    rules: &[ColumnRule],
) -> (Vec<ColumnGroup>, std::collections::HashSet<usize>) {
    let mut consumed = std::collections::HashSet::new();
    let mut grouped_entries: std::collections::BTreeMap<String, Vec<ColumnGroupEntry>> =
        std::collections::BTreeMap::new();

    let name_to_idx: std::collections::HashMap<&str, usize> = schema
        .fields()
        .iter()
        .enumerate()
        .filter(|(i, _)| !excluded.contains(i))
        .map(|(i, f)| (f.name().as_str(), i))
        .collect();

    /// Try to match all suffixes against raw column names, returning `(prefix, col_indices)` pairs.
    fn try_match_raw(
        suffixes: &[String],
        name_to_idx: &std::collections::HashMap<&str, usize>,
        consumed: &std::collections::HashSet<usize>,
    ) -> Vec<(String, Vec<usize>)> {
        if suffixes.is_empty() {
            return vec![];
        }
        let first_suffix = &suffixes[0];
        let mut matches = vec![];
        for (&name, &idx) in name_to_idx {
            if consumed.contains(&idx) {
                continue;
            }
            let Some(prefix) = name.strip_suffix(first_suffix.as_str()) else {
                continue;
            };
            let mut col_indices = vec![idx];
            let mut all_found = true;
            for suffix in &suffixes[1..] {
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
                matches.push((prefix.to_owned(), col_indices));
            }
        }
        matches
    }

    for rule in rules {
        if let ColumnMapping::Transform { rotation_suffixes } = &rule.mapping {
            use re_sdk_types::archetypes::Transform3D;

            let trans_matches = try_match_raw(&rule.suffixes, &name_to_idx, &consumed);
            // Consume translation columns first.
            for (_, indices) in &trans_matches {
                for &ci in indices {
                    consumed.insert(ci);
                }
            }
            let rot_matches = try_match_raw(rotation_suffixes, &name_to_idx, &consumed);
            for (_, indices) in &rot_matches {
                for &ci in indices {
                    consumed.insert(ci);
                }
            }

            // Join on prefix.
            let mut rot_by_prefix: std::collections::HashMap<String, Vec<usize>> =
                rot_matches.into_iter().collect();

            for (prefix, trans_indices) in trans_matches {
                if let Some(rot_indices) = rot_by_prefix.remove(&prefix) {
                    grouped_entries
                        .entry(prefix)
                        .or_default()
                        .push(ColumnGroupEntry::Transform {
                            translation_col_indices: trans_indices,
                            rotation_col_indices: rot_indices,
                            translation_descriptor: Transform3D::descriptor_translation(),
                            rotation_descriptor: Transform3D::descriptor_quaternion(),
                            field_name: String::new(),
                        });
                } else {
                    // Unconsume unmatched translation columns.
                    for &ci in &trans_indices {
                        consumed.remove(&ci);
                    }
                }
            }
            // Unconsume unmatched rotation columns.
            for indices in rot_by_prefix.values() {
                for &ci in indices {
                    consumed.remove(&ci);
                }
            }
        } else {
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
                    let entry = match &rule.mapping {
                        ColumnMapping::Component { descriptor } => ColumnGroupEntry::Component {
                            col_indices,
                            descriptor: descriptor.clone(),
                            field_name: prefix.to_owned(),
                        },
                        ColumnMapping::Scalars { names } => ColumnGroupEntry::ScalarGroup {
                            col_indices,
                            names: names.clone(),
                            field_name: prefix.to_owned(),
                        },
                        ColumnMapping::Transform { .. } => unreachable!(),
                    };
                    grouped_entries
                        .entry(prefix.to_owned())
                        .or_default()
                        .push(entry);
                }
            }
        }
    }

    let groups = grouped_entries
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

/// Derive the struct field name from sub-prefix and optional override.
///
/// When `field_name_override` is `Some`, `suffix_fallback` is ignored entirely —
/// the override replaces whatever the `suffix_fallback` would have contributed.
fn derive_field_name(
    sub_prefix: &str,
    suffix_fallback: &str,
    field_name_override: Option<&str>,
) -> String {
    // Treat empty override as no override.
    let field_name_override = field_name_override.filter(|s| !s.is_empty());

    match field_name_override {
        Some(ovr) => {
            let clean_ovr = ovr.strip_prefix('_').unwrap_or(ovr);
            if sub_prefix.is_empty() {
                clean_ovr.to_owned()
            } else {
                format!("{sub_prefix}{ovr}")
            }
        }
        None => {
            if sub_prefix.is_empty() {
                if suffix_fallback.is_empty() {
                    String::new()
                } else {
                    suffix_fallback.to_owned()
                }
            } else {
                sub_prefix.to_owned()
            }
        }
    }
}

/// Derive a field name from the common prefix of suffix patterns.
///
/// For suffixes like `["_pos_x", "_pos_y", "_pos_z"]`, returns `"pos"`.
/// For suffixes like `["_x", "_y", "_z"]`, returns `""`.
fn suffix_common_prefix(suffixes: &[String]) -> String {
    let stripped: Vec<&str> = suffixes
        .iter()
        .map(|s| s.strip_prefix('_').unwrap_or(s.as_str()))
        .collect();
    if stripped.is_empty() {
        return String::new();
    }
    let first = stripped[0].as_bytes();
    let mut len = first.len();
    for s in &stripped[1..] {
        let b = s.as_bytes();
        len = len.min(b.len());
        for i in 0..len {
            if first[i] != b[i] {
                len = i;
                break;
            }
        }
    }
    let prefix = &stripped[0][..len];
    prefix.strip_suffix('_').unwrap_or(prefix).to_owned()
}

/// Log a warning if an earlier rule may shadow a later, more specific rule.
fn warn_shadowed_rules(rules: &[ColumnRule]) {
    for i in 0..rules.len() {
        for j in (i + 1)..rules.len() {
            let a = &rules[i].suffixes;
            let b = &rules[j].suffixes;
            if a.len() == b.len() {
                let shadows = a.iter().zip(b.iter()).all(|(sa, sb)| {
                    let sa = sa.strip_prefix('_').unwrap_or(sa.as_str());
                    let sb = sb.strip_prefix('_').unwrap_or(sb.as_str());
                    sb.ends_with(sa)
                });
                if shadows {
                    re_log::warn_once!(
                        "Column rule {} (suffixes {:?}) may shadow rule {} (suffixes {:?}). \
                         Consider reordering so more specific rules come first.",
                        i,
                        a,
                        j,
                        b
                    );
                }
            }
        }
    }
}
