//! The server matches chunk indexing policy entries exactly, so the client resolves entity path
//! wildcards against the dataset schema.

use std::collections::BTreeSet;

use pyo3::exceptions::PyValueError;
use pyo3::types::PyTuple;
use pyo3::{
    FromPyObject, IntoPyObjectExt as _, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods,
};
use re_log_types::{EntityPath, EntityPathFilter, ResolvedEntityPathFilter, RuleEffect};
use re_protos::cloud::v1alpha1::ext::{ChunkIndexingPolicy, IndexingPolicySelection};

use super::PySchemaInternal;

/// One dimension of a policy as Python passes it: a list of names, or the literal `"all"`.
#[derive(FromPyObject)]
pub enum PyNameSelection {
    #[pyo3(transparent, annotation = "list[str]")]
    Explicit(Vec<String>),
    #[pyo3(transparent, annotation = "str")]
    Literal(String),
}

impl PyNameSelection {
    fn into_selection(self) -> Result<IndexingPolicySelection, String> {
        match self {
            Self::Explicit(names) => Ok(IndexingPolicySelection::Explicit(
                names.into_iter().collect(),
            )),
            Self::Literal(literal) if literal == "all" => Ok(IndexingPolicySelection::All),
            Self::Literal(other) => Err(format!(
                "expected a list of names or \"all\", got {other:?}"
            )),
        }
    }

    /// `"all"`, or the names as a sorted list.
    fn to_py(selection: &IndexingPolicySelection, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match selection {
            IndexingPolicySelection::All => "all".into_py_any(py),
            IndexingPolicySelection::Explicit(names) => {
                names.iter().collect::<Vec<_>>().into_py_any(py)
            }
        }
    }
}

/// Converts the Python policy dimensions into a [`ChunkIndexingPolicy`].
///
/// Raises `ValueError` for an entity path rule (a wildcard or a `+`/`-` prefix) and for any string
/// other than `"all"`. Callers resolve rules with `ChunkIndexingPolicy.match` first.
pub fn to_policy(
    entity_paths: PyNameSelection,
    timelines: PyNameSelection,
) -> PyResult<ChunkIndexingPolicy> {
    policy_from_selections(entity_paths, timelines).map_err(PyValueError::new_err)
}

/// [`to_policy`] without the Python error type, so unit tests need no Python library.
fn policy_from_selections(
    entity_paths: PyNameSelection,
    timelines: PyNameSelection,
) -> Result<ChunkIndexingPolicy, String> {
    if let PyNameSelection::Explicit(entity_paths) = &entity_paths
        && let Some(entry) = find_wildcard(entity_paths)
    {
        return Err(format!(
            "entity path {entry:?} in the chunk indexing policy is a wildcard; resolve it with \
             `ChunkIndexingPolicy.match(dataset.schema())` first"
        ));
    }
    Ok(ChunkIndexingPolicy {
        entity_paths: entity_paths.into_selection()?,
        timelines: timelines.into_selection()?,
    })
}

/// Converts `policy` to the `(entity_paths, timelines)` tuple Python uses.
pub fn to_py_tuple(py: Python<'_>, policy: &ChunkIndexingPolicy) -> PyResult<Py<PyTuple>> {
    let entity_paths = PyNameSelection::to_py(&policy.entity_paths, py)?;
    let timelines = PyNameSelection::to_py(&policy.timelines, py)?;
    Ok(PyTuple::new(py, [entity_paths, timelines])?.unbind())
}

fn find_wildcard(entity_paths: &[String]) -> Option<&str> {
    entity_paths
        .iter()
        .find(|entry| entry.trim_start().starts_with(['+', '-']) || has_unescaped_star(entry))
        .map(String::as_str)
}

#[pyclass(
    frozen,
    name = "ChunkIndexingPolicyInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyChunkIndexingPolicyInternal;

#[pymethods]
impl PyChunkIndexingPolicyInternal {
    /// Resolves entity path rules against `schema`.
    #[staticmethod]
    fn match_entity_paths(
        schema: PyRef<'_, PySchemaInternal>,
        entity_paths: Vec<String>,
    ) -> Vec<String> {
        let schema_entity_paths = schema
            .columns
            .component_columns()
            .map(|col| col.entity_path.clone());
        match_entity_paths(schema_entity_paths, &entity_paths)
    }
}

/// Resolves `rules`, one `EntityPathFilter` rule per entry, into a sorted list of entity paths.
///
/// Keeps exact includes even if the schema lacks them.
fn match_entity_paths(
    schema_entity_paths: impl IntoIterator<Item = EntityPath>,
    rules: &[String],
) -> Vec<String> {
    let filter = EntityPathFilter::from_query_expressions(rules.iter().map(String::as_str))
        .resolve_without_substitutions();

    matching_entity_paths(schema_entity_paths, std::slice::from_ref(&filter))
        .iter()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Returns the schema paths and exact includes that every filter in `filters` matches.
///
/// Exact includes count even if the schema lacks them.
pub fn matching_entity_paths(
    schema_entity_paths: impl IntoIterator<Item = EntityPath>,
    filters: &[ResolvedEntityPathFilter],
) -> BTreeSet<EntityPath> {
    let exact_includes = filters.iter().flat_map(|filter| {
        filter
            .rules()
            .filter(|(rule, effect)| {
                **effect == RuleEffect::Include && !rule.rule.include_subtree()
            })
            .map(|(rule, _)| rule.resolved_path.clone())
    });

    std::iter::chain(schema_entity_paths, exact_includes)
        .filter(|path| filters.iter().all(|filter| filter.matches(path)))
        .collect()
}

/// Returns true if `entry` has a `*` without a preceding backslash.
///
/// `\*` is a literal `*` in an entity path part.
fn has_unescaped_star(entry: &str) -> bool {
    let mut chars = entry.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '*' => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn schema_paths(items: &[&str]) -> Vec<EntityPath> {
        items.iter().map(|s| EntityPath::from(*s)).collect()
    }

    #[test]
    fn entity_path_rules_include_and_exclude() {
        let schema = schema_paths(&["/world", "/world/car", "/world/raw/lidar", "/robot"]);
        let matched = match_entity_paths(
            schema,
            &strings(&["/world/**", "-/world/raw/**", "/not/yet/logged"]),
        );
        assert_eq!(
            matched,
            strings(&["/not/yet/logged", "/world", "/world/car"])
        );

        let matched = match_entity_paths(schema_paths(&["/a"]), &strings(&["/a/b", "-/a/**"]));
        assert_eq!(matched, strings(&["/a/b"]));

        let matched = match_entity_paths(schema_paths(&[]), &strings(&["/a", "-/a"]));
        assert!(matched.is_empty(), "{matched:?}");
    }

    #[test]
    fn entity_paths_skip_properties_unless_named() {
        let schema = schema_paths(&["/a", "/__properties"]);
        assert_eq!(
            match_entity_paths(schema.clone(), &strings(&["/**"])),
            strings(&["/a"])
        );
        assert_eq!(
            match_entity_paths(schema, &strings(&["/**", "/__properties/**"])),
            strings(&["/__properties", "/a"])
        );
    }

    #[test]
    fn guard_rejects_only_rules_and_unescaped_stars() {
        for path in ["/a/**", "/a/*", "-/a", "+/a", " -/a"] {
            assert_eq!(find_wildcard(&strings(&[path])), Some(path));
        }

        assert_eq!(find_wildcard(&strings(&["/a", r"/a/\*", "b/c"])), None);

        let policy = policy_from_selections(
            PyNameSelection::Literal("all".to_owned()),
            PyNameSelection::Explicit(vec![]),
        )
        .expect("\"all\" is not a wildcard");
        assert_eq!(policy.entity_paths, IndexingPolicySelection::All);
    }
}
