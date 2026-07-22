//! Metadata-only recursive walk of an HDF5 file.
//!
//! The single source of truth for descending the file, shared by planning,
//! validation, and the `list_*` metadata accessors. Uses only metadata calls
//! (`groups`/`datasets`/`shape`/`dtype`/`attrs`) — never reads dataset values —
//! and drops every borrowed handle before returning owned data.

use hdf5_pure::{AttrValue, DType, File, Group};

use crate::error::Hdf5Error;

/// An absolute in-file HDF5 object path, stored as its name segments (the
/// root group is the empty path).
///
/// HDF5 object names cannot contain `/`, so segment ↔ string conversions are
/// lossless. `Display` yields the absolute form (`/observations/qpos`, `/` for
/// the root); [`Self::as_hdf5`] the relative form `hdf5_pure` navigates by.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct H5Path {
    segments: Vec<String>,
}

impl H5Path {
    pub fn root() -> Self {
        Self::default()
    }

    /// Parse a user-provided object path; leading/trailing/repeated slashes
    /// are ignored (`"/"`, `""` → the root).
    pub fn parse(path: &str) -> Self {
        Self {
            segments: path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        }
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn child(&self, name: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(name.to_owned());
        Self { segments }
    }

    pub fn parent(&self) -> Self {
        Self {
            segments: self.segments[..self.segments.len().saturating_sub(1)].to_vec(),
        }
    }

    /// The object's own name (`None` for the root).
    pub fn leaf(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(String::as_str)
    }

    /// True if `self` is `ancestor` or lies inside its subtree.
    pub fn is_or_under(&self, ancestor: &Self) -> bool {
        self.segments.len() >= ancestor.segments.len()
            && self.segments[..ancestor.segments.len()] == ancestor.segments[..]
    }

    /// Slash-joined relative form as accepted by `hdf5_pure::File::group`/`dataset`.
    pub fn as_hdf5(&self) -> String {
        self.segments.join("/")
    }
}

impl std::fmt::Display for H5Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_root() {
            f.write_str("/")
        } else {
            write!(f, "/{}", self.segments.join("/"))
        }
    }
}

/// Structural description of a single dataset.
pub(crate) struct DatasetDesc {
    pub path: H5Path,
    pub shape: Vec<u64>,
    pub dtype: DType,
}

impl DatasetDesc {
    /// The dataset's leaf name → component / struct-field name.
    pub fn name(&self) -> &str {
        self.path.leaf().expect("a dataset path is never the root")
    }
}

/// The attributes attached to one object (group or dataset), sorted by name.
pub(crate) struct AttrDesc {
    pub path: H5Path,
    pub attrs: Vec<(String, AttrValue)>,
}

/// Everything a metadata walk discovers, in deterministic order.
#[derive(Default)]
pub(crate) struct Walk {
    /// Every group strictly below the walk's start.
    pub groups: Vec<H5Path>,

    pub datasets: Vec<DatasetDesc>,
    pub attrs: Vec<AttrDesc>,
}

/// The `ignore_datasets` entries; a group entry excludes its whole subtree.
pub(crate) struct IgnoreSet {
    entries: Vec<H5Path>,
}

impl IgnoreSet {
    pub fn new(ignore_paths: &[String]) -> Self {
        Self {
            entries: ignore_paths
                .iter()
                .map(|path| H5Path::parse(path))
                .collect(),
        }
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn is_ignored(&self, path: &H5Path) -> bool {
        self.entries.iter().any(|entry| path.is_or_under(entry))
    }
}

/// Resolve `path` to a group handle; the root is special-cased through
/// `File::root()` since `File::group("")` behavior is unspecified.
pub(crate) fn open_group<'f>(file: &'f File, path: &H5Path) -> Result<Group<'f>, Hdf5Error> {
    if path.is_root() {
        Ok(file.root())
    } else {
        file.group(&path.as_hdf5())
            .map_err(|_err| Hdf5Error::ObjectNotFound {
                path: path.to_string(),
            })
    }
}

/// Recursively walk from `start`, honoring `ignore`.
pub(crate) fn walk(file: &File, start: &H5Path, ignore: &IgnoreSet) -> Result<Walk, Hdf5Error> {
    re_tracing::profile_function!();

    let group = open_group(file, start)?;
    let mut out = Walk::default();
    walk_group(&group, start, ignore, &mut out)?;
    Ok(out)
}

fn walk_group(
    group: &Group<'_>,
    path: &H5Path,
    ignore: &IgnoreSet,
    out: &mut Walk,
) -> Result<(), Hdf5Error> {
    let group_err = |source| Hdf5Error::metadata(path, source);

    push_attrs(group.attrs().map_err(group_err)?, path.clone(), out);

    // Sort child names so output order is deterministic regardless of on-disk layout.
    let mut dataset_names = group.datasets().map_err(group_err)?;
    dataset_names.sort();
    for name in dataset_names {
        let dataset_path = path.child(&name);
        if ignore.is_ignored(&dataset_path) {
            continue;
        }

        let ds_err = |source| Hdf5Error::metadata(&dataset_path, source);
        let dataset = group.dataset(&name).map_err(ds_err)?;
        push_attrs(dataset.attrs().map_err(ds_err)?, dataset_path.clone(), out);
        out.datasets.push(DatasetDesc {
            shape: dataset.shape().map_err(ds_err)?,
            dtype: dataset.dtype().map_err(ds_err)?,
            path: dataset_path,
        });
    }

    let mut group_names = group.groups().map_err(group_err)?;
    group_names.sort();
    for name in group_names {
        let group_path = path.child(&name);
        if ignore.is_ignored(&group_path) {
            continue;
        }

        let subgroup = group
            .group(&name)
            .map_err(|source| Hdf5Error::metadata(&group_path, source))?;
        out.groups.push(group_path.clone());
        walk_group(&subgroup, &group_path, ignore, out)?;
    }

    Ok(())
}

fn push_attrs(attrs: std::collections::HashMap<String, AttrValue>, path: H5Path, out: &mut Walk) {
    if attrs.is_empty() {
        return;
    }
    let mut attrs: Vec<(String, AttrValue)> = attrs.into_iter().collect();
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    out.attrs.push(AttrDesc { path, attrs });
}
