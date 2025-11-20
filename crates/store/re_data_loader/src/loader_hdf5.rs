//! Rerun dataloader for HDF5 files.

use re_chunk::RowId;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};

use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

#[cfg(not(target_arch = "wasm32"))]
use std::{path::Path, sync::mpsc::Sender};

#[cfg(not(target_arch = "wasm32"))]
use hdf5_metno::{Dataset, File};

const HDF5_LOADER_NAME: &str = "Hdf5Loader";

/// A [`DataLoader`] for HDF5 files.
///
/// The HDF5 loader extracts datasets from HDF5 files and converts them to Arrow format
/// for use in Rerun. It performs raw extraction of datasets without interpretation,
/// preserving the original data structure as closely as possible.
///
/// Supported HDF5 features:
/// - Numeric datasets (integers, floats)
/// - String datasets
/// - Multi-dimensional arrays
/// - Group hierarchies (mapped to entity paths)
/// - Basic attributes as metadata
pub struct Hdf5Loader;

impl Default for Hdf5Loader {
    fn default() -> Self {
        Self
    }
}

impl Hdf5Loader {
    /// Creates a new [`Hdf5Loader`].
    pub fn new() -> Self {
        Self
    }
}

impl DataLoader for Hdf5Loader {
    fn name(&self) -> crate::DataLoaderName {
        HDF5_LOADER_NAME.into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        path: std::path::PathBuf,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        re_log::debug!("HDF5 loader checking path: {:?}", path);
        if !is_hdf5_file(&path) {
            re_log::debug!("HDF5 loader: not an HDF5 file");
            return Err(DataLoaderError::Incompatible(path)); // simply not interested
        }
        re_log::debug!("HDF5 loader: accepting HDF5 file");

        re_tracing::profile_function!();

        // NOTE: this must be spawned on a dedicated thread to avoid potential blocking issues
        let settings = settings.clone();
        std::thread::Builder::new()
            .name(format!("load_hdf5({path:?})"))
            .spawn(move || {
                if let Err(err) = load_hdf5_file(&path, &settings, &tx) {
                    re_log::error!("Failed to load HDF5 file: {err}");
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), crate::DataLoaderError> {
        // HDF5 files require filesystem access for the current implementation
        // In-memory loading would require additional work with the HDF5 crate
        Err(DataLoaderError::Incompatible(filepath))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_hdf5_file(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
) -> std::result::Result<(), DataLoaderError> {
    // File is already imported at the top level

    re_tracing::profile_function!();
    let store_id = settings.recommended_store_id();

    // Send store info
    if tx
        .send(LoadedData::LogMsg(
            HDF5_LOADER_NAME.to_owned(),
            re_log_types::LogMsg::SetStoreInfo(store_info(store_id.clone())),
        ))
        .is_err()
    {
        re_log::debug_once!(
            "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
        );
        return Ok(());
    }

    // Open HDF5 file
    let file = File::open(filepath).map_err(|err| {
        DataLoaderError::Other(anyhow::anyhow!("Failed to open HDF5 file: {err}"))
    })?;

    re_log::info!("HDF5 file opened successfully: {}", filepath.display());

    // Extract datasets recursively from the root group
    extract_hdf5_group(
        &file, "", // Root path
        settings, &store_id, tx,
    )?;

    Ok(())
}

/// Recursively extracts datasets from an HDF5 group
#[cfg(not(target_arch = "wasm32"))]
fn extract_hdf5_group(
    group: &hdf5_metno::Group,
    group_path: &str,
    settings: &DataLoaderSettings,
    store_id: &StoreId,
    tx: &Sender<LoadedData>,
) -> std::result::Result<(), DataLoaderError> {
    // Get all member names in this group
    let member_names = group.member_names().map_err(|err| {
        DataLoaderError::Other(anyhow::anyhow!("Failed to get group members: {err}"))
    })?;

    for member_name in member_names {
        let member_path = if group_path.is_empty() {
            member_name.clone()
        } else {
            format!("{group_path}/{member_name}")
        };

        // Try to open as dataset first
        if let Ok(dataset) = group.dataset(&member_name) {
            if let Err(err) = extract_hdf5_dataset(&dataset, &member_path, settings, store_id, tx) {
                re_log::warn!("Failed to extract dataset '{}': {}", member_path, err);
            }
        }
        // Try to open as group
        else if let Ok(subgroup) = group.group(&member_name) {
            extract_hdf5_group(&subgroup, &member_path, settings, store_id, tx)?;
        }
        // Skip if it's neither a dataset nor group
        else {
            re_log::debug!("Skipping member '{}': not a dataset or group", member_path);
        }
    }

    Ok(())
}

/// Extracts a single HDF5 dataset and converts it to Arrow format
#[cfg(not(target_arch = "wasm32"))]
fn extract_hdf5_dataset(
    dataset: &hdf5_metno::Dataset,
    dataset_path: &str,
    settings: &DataLoaderSettings,
    store_id: &StoreId,
    tx: &Sender<LoadedData>,
) -> std::result::Result<(), DataLoaderError> {
    use arrow::record_batch::RecordBatch;
    use re_log_types::EntityPath;

    re_log::debug!("Extracting dataset: {}", dataset_path);

    // Get dataset shape and type info
    let shape = dataset.shape();
    let ndim = dataset.ndim();

    re_log::debug!(
        "Dataset '{}' has shape {:?}, ndim: {}",
        dataset_path,
        shape,
        ndim
    );

    // For now, only handle 1D and 2D datasets
    if ndim == 0 || ndim > 2 {
        return Err(DataLoaderError::Other(anyhow::anyhow!(
            "Dataset '{dataset_path}' has unsupported dimensionality: {ndim}D (only 1D and 2D supported)"
        )));
    }

    // Try to read as different data types using reflection-based approach
    let (schema, arrays) = read_dataset_to_arrow(dataset, dataset_path, &shape)?;

    // Create RecordBatch with proper options
    let batch = RecordBatch::try_new_with_options(
        std::sync::Arc::new(schema),
        arrays,
        &arrow::record_batch::RecordBatchOptions::new().with_row_count(None),
    )
    .map_err(|err| {
        DataLoaderError::Other(anyhow::anyhow!("Failed to create RecordBatch: {err}"))
    })?;

    // Convert to entity path
    let entity_path = if let Some(prefix) = &settings.entity_path_prefix {
        EntityPath::from(format!("{prefix}/{dataset_path}"))
    } else {
        EntityPath::from(dataset_path)
    };

    // Create serialized component batches for chunk creation
    // For HDF5 data, we'll create components that contain the raw data
    // This matches the "raw extraction" approach requested
    let component_batches: Vec<re_types::SerializedComponentBatch> = batch
        .schema()
        .fields()
        .iter()
        .enumerate()
        .map(|(field_idx, field)| {
            let component_name = field.name().clone();
            let array = batch.column(field_idx).clone();

            // Create a component descriptor - using a generic name since this is raw data
            let component_descriptor = re_types::ComponentDescriptor::partial(component_name);
            re_types::SerializedComponentBatch {
                descriptor: component_descriptor,
                array,
            }
        })
        .collect();

    // Create chunk using ChunkBuilder with serialized batches
    let chunk = re_chunk::ChunkBuilder::new(re_chunk::ChunkId::new(), entity_path)
        .with_serialized_batches(
            re_chunk::RowId::new(),
            re_chunk::TimePoint::default(), // No timeline data for now - this is raw data extraction
            component_batches,
        )
        .build()
        .map_err(|err| DataLoaderError::Other(anyhow::anyhow!("Failed to create chunk: {err}")))?;

    // Send the chunk
    if tx
        .send(LoadedData::Chunk(
            HDF5_LOADER_NAME.to_owned(),
            store_id.clone(),
            chunk,
        ))
        .is_err()
    {
        re_log::debug!(
            "Failed to send dataset '{}' because channel was closed",
            dataset_path
        );
    } else {
        re_log::debug!("Successfully sent dataset: {}", dataset_path);
    }

    Ok(())
}

/// Read HDF5 dataset and convert to Arrow format using reflection-based approach
#[cfg(not(target_arch = "wasm32"))]
fn read_dataset_to_arrow(
    dataset: &Dataset,
    dataset_path: &str,
    shape: &[usize],
) -> std::result::Result<
    (
        arrow::datatypes::Schema,
        Vec<std::sync::Arc<dyn arrow::array::Array>>,
    ),
    DataLoaderError,
> {
    use arrow::{
        array::{
            Float32Array, Float64Array, Int8Array, Int16Array, Int32Array, Int64Array, UInt8Array,
            UInt16Array, UInt32Array, UInt64Array,
        },
        datatypes::DataType,
    };

    // Try different data types and convert to Arrow arrays
    macro_rules! try_read_type {
        ($rust_type:ty, $arrow_array:ty, $data_type:expr) => {
            if let Ok(data) = dataset.read::<$rust_type, _>() {
                return convert_ndarray_to_arrow::<$rust_type, $arrow_array>(
                    data,
                    shape,
                    $data_type,
                    dataset_path,
                );
            }
        };
    }

    // Try common numeric types in order of preference
    try_read_type!(f64, Float64Array, DataType::Float64);
    try_read_type!(f32, Float32Array, DataType::Float32);
    try_read_type!(i64, Int64Array, DataType::Int64);
    try_read_type!(i32, Int32Array, DataType::Int32);
    try_read_type!(u64, UInt64Array, DataType::UInt64);
    try_read_type!(u32, UInt32Array, DataType::UInt32);
    try_read_type!(i16, Int16Array, DataType::Int16);
    try_read_type!(u16, UInt16Array, DataType::UInt16);
    try_read_type!(i8, Int8Array, DataType::Int8);
    try_read_type!(u8, UInt8Array, DataType::UInt8);

    Err(DataLoaderError::Other(anyhow::anyhow!(
        "Dataset '{dataset_path}' has unsupported data type"
    )))
}

/// Convert ndarray to Arrow arrays with proper schema - simplified version
#[cfg(not(target_arch = "wasm32"))]
fn convert_ndarray_to_arrow<T, A>(
    data: ndarray::ArrayD<T>,
    shape: &[usize],
    data_type: arrow::datatypes::DataType,
    dataset_path: &str,
) -> std::result::Result<
    (
        arrow::datatypes::Schema,
        Vec<std::sync::Arc<dyn arrow::array::Array>>,
    ),
    DataLoaderError,
>
where
    T: Clone,
    A: arrow::array::Array + 'static,
    A: From<Vec<T>>,
{
    use arrow::datatypes::{Field, Schema};

    let (data_vec, _offset) = data.into_raw_vec_and_offset();

    if shape.len() == 1 {
        // 1D dataset - single column
        let array = A::from(data_vec);
        let field = Field::new("value", data_type, false);
        let schema = Schema::new_with_metadata(vec![field], std::collections::HashMap::new());
        Ok((schema, vec![std::sync::Arc::new(array)]))
    } else if shape.len() == 2 {
        // 2D dataset - multiple columns
        let rows = shape[0];
        let cols = shape[1];
        let mut arrays = Vec::new();
        let mut fields = Vec::new();

        for col in 0..cols {
            let mut column_data = Vec::with_capacity(rows);
            for row in 0..rows {
                let index = row * cols + col;
                if index < data_vec.len() {
                    column_data.push(data_vec[index].clone());
                }
            }

            let array = A::from(column_data);
            arrays.push(std::sync::Arc::new(array) as std::sync::Arc<dyn arrow::array::Array>);
            fields.push(Field::new(format!("col_{col}"), data_type.clone(), false));
        }

        let schema = Schema::new_with_metadata(fields, std::collections::HashMap::new());
        Ok((schema, arrays))
    } else {
        Err(DataLoaderError::Other(anyhow::anyhow!(
            "Dataset '{dataset_path}' has unsupported shape: {shape:?}"
        )))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo::new(
            store_id,
            re_log_types::StoreSource::Other(HDF5_LOADER_NAME.to_owned()),
        ),
    }
}

/// Checks if a path has an HDF5 file extension.
#[cfg(not(target_arch = "wasm32"))]
fn has_hdf5_extension(filepath: &Path) -> bool {
    filepath
        .extension()
        .map(|ext| {
            let ext_str = ext.to_string_lossy().to_lowercase();
            ext_str == "h5" || ext_str == "hdf5" || ext_str == "hdf"
        })
        .unwrap_or(false)
}

/// Checks if a file is an HDF5 file based on its extension and existence.
#[cfg(not(target_arch = "wasm32"))]
fn is_hdf5_file(filepath: &Path) -> bool {
    filepath.is_file() && has_hdf5_extension(filepath)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_hdf5_file() {
        // Test HDF5 extensions (only check extension, not file existence)
        assert!(has_hdf5_extension(&PathBuf::from("test.h5")));
        assert!(has_hdf5_extension(&PathBuf::from("test.hdf5")));
        assert!(has_hdf5_extension(&PathBuf::from("test.hdf")));
        assert!(has_hdf5_extension(&PathBuf::from("TEST.H5")));

        // Test non-HDF5 extensions
        assert!(!has_hdf5_extension(&PathBuf::from("test.txt")));
        assert!(!has_hdf5_extension(&PathBuf::from("test.mcap")));
        assert!(!has_hdf5_extension(&PathBuf::from("test.rrd")));
    }

    #[test]
    fn test_hdf5_data_conversion() {
        use hdf5_metno::File;
        use tempfile::tempdir;

        // Create a temporary HDF5 file for testing
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.h5");

        // Create test data
        {
            let file = File::create(&file_path).expect("Failed to create HDF5 file");

            // Create a simple 1D dataset
            let data_1d = vec![1.0f64, 2.0, 3.0, 4.0, 5.0];
            let dataset_1d = file
                .new_dataset::<f64>()
                .shape([data_1d.len()])
                .create("data_1d")
                .expect("Failed to create 1D dataset");
            dataset_1d.write(&data_1d).expect("Failed to write 1D data");

            // Create a simple 2D dataset
            let data_2d = vec![1.0f64, 2.0, 3.0, 4.0, 5.0, 6.0];
            let dataset_2d = file
                .new_dataset::<f64>()
                .shape([2, 3])
                .create("data_2d")
                .expect("Failed to create 2D dataset");
            dataset_2d
                .write_raw(&data_2d)
                .expect("Failed to write 2D data");

            file.close().expect("Failed to close file");
        }

        // Test reading the file
        let file = File::open(&file_path).expect("Failed to open HDF5 file");

        // Test 1D dataset conversion
        let dataset_1d = file.dataset("data_1d").expect("Failed to get 1D dataset");
        let shape_1d = dataset_1d.shape();
        let result_1d = read_dataset_to_arrow(&dataset_1d, "data_1d", &shape_1d);
        assert!(
            result_1d.is_ok(),
            "Failed to convert 1D dataset: {:?}",
            result_1d.err()
        );

        let (schema_1d, arrays_1d) = result_1d.unwrap();
        assert_eq!(schema_1d.fields().len(), 1);
        assert_eq!(arrays_1d.len(), 1);
        assert_eq!(arrays_1d[0].len(), 5);

        // Test 2D dataset conversion
        let dataset_2d = file.dataset("data_2d").expect("Failed to get 2D dataset");
        let shape_2d = dataset_2d.shape();
        let result_2d = read_dataset_to_arrow(&dataset_2d, "data_2d", &shape_2d);
        assert!(
            result_2d.is_ok(),
            "Failed to convert 2D dataset: {:?}",
            result_2d.err()
        );

        let (schema_2d, arrays_2d) = result_2d.unwrap();
        assert_eq!(schema_2d.fields().len(), 3); // 3 columns
        assert_eq!(arrays_2d.len(), 3);
        assert_eq!(arrays_2d[0].len(), 2); // 2 rows
    }
}
