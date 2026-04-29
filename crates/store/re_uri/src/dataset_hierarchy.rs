/// Separator used in folder path prefixes and in dataset names to denote hierarchy
/// levels (e.g. `"project.subdir.leaf"`).
pub const DATASET_HIERARCHY_SEPARATOR: char = '.';

/// Split an entry name into hierarchy path segments.
///
/// Trailing separators are part of the leaf name, not hierarchy delimiters:
/// `"a.b."` becomes `["a", "b."]`, and `"a."` becomes `["a."]`.
pub fn split_dataset_hierarchy_path(path: &str) -> impl Iterator<Item = &str> {
    let hierarchy_path = path.trim_end_matches(DATASET_HIERARCHY_SEPARATOR);

    let (parents, leaf) = if let Some((parents, _leaf_without_trailing_separators)) =
        hierarchy_path.rsplit_once(DATASET_HIERARCHY_SEPARATOR)
    {
        let leaf_start = parents.len() + DATASET_HIERARCHY_SEPARATOR.len_utf8();
        (Some(parents), &path[leaf_start..])
    } else {
        (None, path)
    };

    parents
        .into_iter()
        .flat_map(|parents| {
            parents
                .split(DATASET_HIERARCHY_SEPARATOR)
                .filter(|s| !s.is_empty())
        })
        .chain(std::iter::once(leaf))
}

/// Returns the leaf segment of an entry name using [`split_dataset_hierarchy_path`] semantics.
pub fn dataset_hierarchy_leaf_name(path: &str) -> &str {
    split_dataset_hierarchy_path(path).last().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{dataset_hierarchy_leaf_name, split_dataset_hierarchy_path};

    #[test]
    fn split_dataset_hierarchy_path_keeps_trailing_dots_in_leaf() {
        let split = |path| split_dataset_hierarchy_path(path).collect::<Vec<_>>();
        assert_eq!(split("a.b.c"), vec!["a", "b", "c"]);
        assert_eq!(split("a.b."), vec!["a", "b."]);
        assert_eq!(split("a."), vec!["a."]);
        assert_eq!(split("a.b.."), vec!["a", "b.."]);
    }

    #[test]
    fn dataset_hierarchy_leaf_name_keeps_trailing_dots() {
        assert_eq!(dataset_hierarchy_leaf_name("a.b.c"), "c");
        assert_eq!(dataset_hierarchy_leaf_name("a.b."), "b.");
        assert_eq!(dataset_hierarchy_leaf_name("a."), "a.");
        assert_eq!(dataset_hierarchy_leaf_name("a.b.."), "b..");
    }
}
