//! Phase A: walk the file, resolve and validate the file-wide row count, and
//! build the owned emission plan the streaming iterator consumes.

use arrow::buffer::ScalarBuffer;
use itertools::Itertools as _;
use re_chunk::{EntityPath, Timeline};
use re_log_types::{EntityPathPart, TimeType};

use crate::config::{Hdf5Config, IndexType};
use crate::convert;
use crate::error::Hdf5Error;
use crate::walk::{self, DatasetDesc, H5Path, IgnoreSet, Walk};

/// The entity under which HDF5 attributes are emitted as static components.
const HDF5_PROPERTIES_PART: &str = "__hdf5_properties";

/// One self-contained unit of emission, described by owned data only.
pub(crate) enum EmitUnit {
    /// One object's attributes → one static chunk.
    Attributes {
        entity: EntityPath,
        attrs: Vec<(String, hdf5_pure::AttrValue)>,
    },

    /// A group's 0-D datasets → one static chunk.
    StaticScalars {
        entity: EntityPath,
        datasets: Vec<DatasetDesc>,
    },

    /// A group's row-aligned datasets → timed chunks (row-windowed).
    Data {
        entity: EntityPath,
        datasets: Vec<DatasetDesc>,
    },
}

/// The shared file-wide index and its full time buffer.
pub(crate) struct PlannedTimeline {
    pub timeline: Timeline,
    pub times: ScalarBuffer<i64>,

    /// Whether `times` is sorted, checked once here: every emitted window is a
    /// contiguous slice of it, and a slice of a sorted buffer is sorted, so
    /// windows can assert sortedness without re-scanning.
    pub is_sorted: bool,
}

pub(crate) struct Hdf5Plan {
    pub units: Vec<EmitUnit>,

    /// `Some` iff any `Data` unit exists.
    pub timeline: Option<PlannedTimeline>,
}

/// Resolved file-wide row count and the consumed index dataset, if any.
struct RowResolution {
    /// `None` when the file has no loadable non-scalar dataset (only scalars/attrs).
    row_count: Option<u64>,

    /// What defined the row count, for the `RowAlignment` error message.
    kind: &'static str,

    /// The index dataset; it is consumed and not emitted as data.
    index_path: Option<H5Path>,
}

/// Walk + resolve/validate + read the index + assemble emission units.
pub(crate) fn build_plan(
    file: &hdf5_pure::File,
    config: &Hdf5Config,
) -> Result<Hdf5Plan, Hdf5Error> {
    re_tracing::profile_function!();

    let ignore = IgnoreSet::new(&config.ignore_datasets);
    let walked = walk::walk(file, &H5Path::root(), &ignore)?;
    warn_unsupported(&walked);

    let resolution = resolve_rows(&walked, config)?;
    check_alignment(&walked, &resolution)?;

    let timeline = build_timeline(file, config, &resolution)?;
    let units = build_units(walked, &resolution, &config.entity_path_prefix);

    Ok(Hdf5Plan { units, timeline })
}

/// Metadata-only structural validation (no data reads, no timeline build).
///
/// Shares `walk` + `resolve_rows` + `check_alignment` with [`build_plan`]; the
/// eager call at reader construction turns bad configuration into a prompt error.
pub(crate) fn validate_with_file(
    file: &hdf5_pure::File,
    config: &Hdf5Config,
) -> Result<(), Hdf5Error> {
    re_tracing::profile_function!();

    let ignore = IgnoreSet::new(&config.ignore_datasets);
    let walked = walk::walk(file, &H5Path::root(), &ignore)?;
    let resolution = resolve_rows(&walked, config)?;
    check_alignment(&walked, &resolution)
}

fn warn_unsupported(walked: &Walk) {
    let unsupported = walked
        .datasets
        .iter()
        .filter(|dataset| !convert::supported_dtype(&dataset.dtype))
        .map(|dataset| format!("{} ({})", dataset.path, dataset.dtype))
        .join(", ");

    if !unsupported.is_empty() {
        re_log::warn!("Ignoring HDF5 datasets with unsupported element types: {unsupported}");
    }
}

/// True for the datasets that must align to the file-wide row count: loaded
/// (supported dtype), non-scalar, and not the consumed index.
fn is_aligned_data(dataset: &DatasetDesc, index_path: Option<&H5Path>) -> bool {
    !dataset.shape.is_empty()
        && convert::supported_dtype(&dataset.dtype)
        && Some(&dataset.path) != index_path
}

/// Resolve the file-wide row count (metadata only).
fn resolve_rows(walked: &Walk, config: &Hdf5Config) -> Result<RowResolution, Hdf5Error> {
    if let Some(index) = &config.index_column {
        let target = H5Path::parse(&index.path);
        let desc = walked
            .datasets
            .iter()
            .find(|dataset| dataset.path == target)
            .ok_or_else(|| Hdf5Error::IndexNotFound {
                path: index.path.clone(),
            })?;

        if desc.shape.len() != 1 {
            return Err(Hdf5Error::IndexNotOneDimensional {
                path: index.path.clone(),
                shape: desc.shape.clone(),
            });
        }
        if !convert::is_numeric_dtype(&desc.dtype) {
            return Err(Hdf5Error::IndexNotNumeric {
                path: index.path.clone(),
                dtype: desc.dtype.to_string(),
            });
        }

        Ok(RowResolution {
            row_count: Some(desc.shape[0]),
            kind: "index column",
            index_path: Some(desc.path.clone()),
        })
    } else {
        // The reference row count is the first loaded non-scalar dataset's, in
        // deterministic walk order; disagreements surface as alignment offenders.
        let reference = walked
            .datasets
            .iter()
            .find(|dataset| is_aligned_data(dataset, None));

        Ok(RowResolution {
            row_count: reference.map(|dataset| dataset.shape[0]),
            kind: "row_index timeline",
            index_path: None,
        })
    }
}

/// Every loaded, non-scalar, non-index dataset must have `shape[0] == row_count`.
fn check_alignment(walked: &Walk, resolution: &RowResolution) -> Result<(), Hdf5Error> {
    let Some(expected) = resolution.row_count else {
        return Ok(());
    };

    let offenders = walked
        .datasets
        .iter()
        .filter(|dataset| is_aligned_data(dataset, resolution.index_path.as_ref()))
        .filter(|dataset| dataset.shape[0] != expected)
        .map(|dataset| format!("{} (shape {:?})", dataset.path, dataset.shape))
        .join(", ");

    if offenders.is_empty() {
        Ok(())
    } else {
        Err(Hdf5Error::RowAlignment {
            kind: resolution.kind,
            expected,
            offenders,
        })
    }
}

/// Build the shared file-wide timeline and its full time buffer.
fn build_timeline(
    file: &hdf5_pure::File,
    config: &Hdf5Config,
    resolution: &RowResolution,
) -> Result<Option<PlannedTimeline>, Hdf5Error> {
    if let (Some(index), Some(path)) = (&config.index_column, &resolution.index_path) {
        let time_type = match index.index_type {
            IndexType::Timestamp(_) => TimeType::TimestampNs,
            IndexType::Duration(_) => TimeType::DurationNs,
            IndexType::Sequence => TimeType::Sequence,
        };
        // The index dataset's leaf name is its natural timeline name.
        let leaf = path.leaf().expect("the index path is never the root");
        let timeline_name = re_chunk::TimelineName::try_new(leaf)
            .map_err(|source| Hdf5Error::invalid_timeline_name(leaf, source))?;
        let times = convert::read_index_to_ns(file, path, index.index_type)?;
        let is_sorted = times.windows(2).all(|times| times[0] <= times[1]);
        Ok(Some(PlannedTimeline {
            timeline: Timeline::new(timeline_name, time_type),
            times,
            is_sorted,
        }))
    } else if let Some(row_count) = resolution.row_count {
        #[expect(clippy::cast_possible_wrap)]
        let times: Vec<i64> = (0..row_count as i64).collect();
        Ok(Some(PlannedTimeline {
            timeline: Timeline::new("row_index", TimeType::Sequence),
            times: ScalarBuffer::from(times),
            is_sorted: true, // sorted by construction
        }))
    } else {
        Ok(None)
    }
}

/// Assemble emission units: attributes first, then per group its timed
/// (`Data`) and static-scalar datasets. The consumed index dataset and
/// unsupported dtypes are dropped; empty units are skipped.
fn build_units(
    walked: Walk,
    resolution: &RowResolution,
    entity_path_prefix: &EntityPath,
) -> Vec<EmitUnit> {
    let mut units = Vec::new();

    for attr in walked.attrs {
        units.push(EmitUnit::Attributes {
            entity: props_entity(entity_path_prefix, &attr.path),
            attrs: attr.attrs,
        });
    }

    // Datasets of the same group are contiguous in walk order.
    for (group_path, group_datasets) in &walked
        .datasets
        .into_iter()
        .chunk_by(|dataset| dataset.path.parent())
    {
        let (scalars, data): (Vec<DatasetDesc>, Vec<DatasetDesc>) = group_datasets
            .filter(|dataset| convert::supported_dtype(&dataset.dtype))
            .filter(|dataset| Some(&dataset.path) != resolution.index_path.as_ref())
            .partition(|dataset| dataset.shape.is_empty());

        let entity = entity_for(entity_path_prefix, &group_path);
        if !data.is_empty() {
            units.push(EmitUnit::Data {
                entity: entity.clone(),
                datasets: data,
            });
        }
        if !scalars.is_empty() {
            units.push(EmitUnit::StaticScalars {
                entity,
                datasets: scalars,
            });
        }
    }

    units
}

// Entity construction is escaping-safe: raw HDF5 names go through
// `EntityPathPart::new`, never string concatenation.

fn entity_for(prefix: &EntityPath, path: &H5Path) -> EntityPath {
    prefix.join(&EntityPath::new(
        path.segments().map(EntityPathPart::new).collect(),
    ))
}

fn props_entity(prefix: &EntityPath, path: &H5Path) -> EntityPath {
    let mut parts = vec![EntityPathPart::new(HDF5_PROPERTIES_PART)];
    parts.extend(path.segments().map(EntityPathPart::new));
    prefix.join(&EntityPath::new(parts))
}
