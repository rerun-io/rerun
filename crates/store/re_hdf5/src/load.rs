//! The crate's public entry points: chunk loading, validation, and the
//! metadata accessors.

use std::path::Path;

use hdf5_pure::AttrValue;
use re_chunk::Chunk;

use crate::config::Hdf5Config;
use crate::convert::DatasetDtype;
use crate::error::Hdf5Error;
use crate::plan;
use crate::streaming;
use crate::walk;

/// Load an HDF5 file and return a lazy iterator of chunks.
///
/// The iterator may yield `Err` for individual chunk-construction failures.
/// Callers who want to continue despite errors should skip `Err` items.
pub fn load_hdf5(
    path: &Path,
    config: &Hdf5Config,
) -> Result<impl Iterator<Item = Result<Chunk, Hdf5Error>> + use<>, Hdf5Error> {
    let file = hdf5_pure::File::open_streaming(path).map_err(Hdf5Error::open)?;
    let plan = plan::build_plan(&file, config)?;
    Ok(streaming::Hdf5ChunkIterator::new(
        file,
        plan,
        config.use_structs,
    ))
}

/// Load HDF5 from in-memory bytes and return a lazy iterator of chunks.
///
/// See [`load_hdf5`] for details on the returned iterator.
pub fn load_hdf5_from_bytes(
    bytes: Vec<u8>,
    config: &Hdf5Config,
) -> Result<impl Iterator<Item = Result<Chunk, Hdf5Error>> + use<>, Hdf5Error> {
    let file = hdf5_pure::File::from_bytes(bytes).map_err(Hdf5Error::open)?;
    let plan = plan::build_plan(&file, config)?;
    Ok(streaming::Hdf5ChunkIterator::new(
        file,
        plan,
        config.use_structs,
    ))
}

/// Metadata-only structural validation of `config` against the file at `path`.
///
/// No dataset values are read and no timeline is built. Callers with a
/// configuration step (e.g. a reader constructor) run this eagerly so bad
/// configuration fails fast.
pub fn validate_layout(path: &Path, config: &Hdf5Config) -> Result<(), Hdf5Error> {
    let file = hdf5_pure::File::open_streaming(path).map_err(Hdf5Error::open)?;
    plan::validate_with_file(&file, config)
}

/// Structural metadata for a single HDF5 dataset.
#[derive(Debug, Clone)]
pub struct DatasetInfo {
    /// Full path of the dataset within the file, e.g. `/observations/qpos`.
    pub path: String,

    /// Dataset dimensions (empty for a 0-D scalar).
    pub shape: Vec<u64>,

    /// Element type (`Display`s as a numpy-style name, e.g. `"uint8"`).
    pub dtype: DatasetDtype,
}

/// Recursively list the full group paths under `group_path` (`"/"` = whole file).
pub fn list_groups(path: &Path, group_path: &str) -> Result<Vec<String>, Hdf5Error> {
    let file = hdf5_pure::File::open_streaming(path).map_err(Hdf5Error::open)?;
    let start = walk::H5Path::parse(group_path);
    let walked = walk::walk(&file, &start, &walk::IgnoreSet::empty())?;
    Ok(walked.groups.iter().map(ToString::to_string).collect())
}

/// Recursively list the datasets under `group_path` with their shape and dtype.
///
/// Metadata only; reflects the raw file, independent of any [`Hdf5Config`].
/// Returns [`Hdf5Error::ObjectNotFound`] if `group_path` is missing.
pub fn list_datasets(path: &Path, group_path: &str) -> Result<Vec<DatasetInfo>, Hdf5Error> {
    let file = hdf5_pure::File::open_streaming(path).map_err(Hdf5Error::open)?;
    let start = walk::H5Path::parse(group_path);
    let walked = walk::walk(&file, &start, &walk::IgnoreSet::empty())?;
    Ok(walked
        .datasets
        .iter()
        .map(|dataset| DatasetInfo {
            path: dataset.path.to_string(),
            shape: dataset.shape.clone(),
            dtype: DatasetDtype::from(&dataset.dtype),
        })
        .collect())
}

/// Read the attributes of the object (group or dataset) at `object_path`
/// (`"/"` = root), sorted by name.
///
/// Returns [`Hdf5Error::ObjectNotFound`] if `object_path` is missing.
pub fn read_attributes(
    path: &Path,
    object_path: &str,
) -> Result<Vec<(String, AttrValue)>, Hdf5Error> {
    let file = hdf5_pure::File::open_streaming(path).map_err(Hdf5Error::open)?;
    let object = walk::H5Path::parse(object_path);

    let not_found = || Hdf5Error::ObjectNotFound {
        path: object_path.to_owned(),
    };

    let meta_err = |source| Hdf5Error::metadata(object_path, source);

    let attrs = if object.is_root() {
        file.root().attrs().map_err(meta_err)?
    } else {
        match file.dataset(&object.as_hdf5()) {
            Ok(dataset) => dataset.attrs().map_err(meta_err)?,
            // The path resolves but is a group, not a dataset.
            Err(hdf5_pure::Error::NotADataset(_)) => file
                .group(&object.as_hdf5())
                .map_err(|_err| not_found())?
                .attrs()
                .map_err(meta_err)?,
            Err(_) => return Err(not_found()),
        }
    };

    let mut attrs: Vec<(String, AttrValue)> = attrs.into_iter().collect();
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(attrs)
}
