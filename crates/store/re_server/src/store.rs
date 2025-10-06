use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    path::Path,
    sync::Arc,
};

use arrow::array::{
    ArrayRef, Int32Array, RecordBatchOptions, StringArray, TimestampNanosecondArray,
};
use arrow::datatypes::{DataType, Field, TimeUnit};
use arrow::{array::RecordBatch, datatypes::Schema};
use datafusion::catalog::TableProvider;
use datafusion::datasource::MemTable;
use datafusion::error::DataFusionError;
use itertools::Itertools as _;
use jiff::Timestamp;
use lance::datafusion::LanceTableProvider;

use re_byte_size::SizeBytes as _;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{EntryId, StoreKind};
use re_protos::{
    cloud::v1alpha1::{
        EntryKind, ScanPartitionTableResponse, SystemTableKind,
        ext::{DatasetEntry, EntryDetails, ProviderDetails as _, SystemTable, TableEntry},
    },
    common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, PartitionId},
};
use re_tuid::Tuid;
use re_types_core::{ComponentBatch as _, Loggable as _};

use crate::entrypoint::NamedPath;

const ENTRIES_TABLE_NAME: &str = "__entries";

#[derive(thiserror::Error, Debug)]
#[expect(clippy::enum_variant_names)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    StoreLoadError(#[from] re_entity_db::StoreLoadError),

    #[error("Entry name '{0}' already exists")]
    DuplicateEntryNameError(String),

    #[error("Entry id '{0}' not found")]
    EntryIdNotFound(EntryId),

    #[error(transparent)]
    DataFusionError(#[from] datafusion::error::DataFusionError),

    #[error("Error loading RRD: {0}")]
    RrdLoadingError(anyhow::Error),
}

impl From<Error> for tonic::Status {
    fn from(value: Error) -> Self {
        match value {
            Error::IoError(err) => Self::internal(format!("IO error: {err:#}")),
            Error::StoreLoadError(err) => Self::internal(format!("Store load error: {err:#}")),
            Error::DuplicateEntryNameError(name) => {
                Self::already_exists(format!("Entry name already exists: {name}"))
            }
            Error::EntryIdNotFound(id) => Self::not_found(format!("Entry ID not found: {id}")),
            Error::DataFusionError(err) => Self::internal(format!("DataFusion error: {err:#}")),
            Error::RrdLoadingError(err) => Self::internal(format!("{err:#}")),
        }
    }
}

pub struct Partition {
    store_handle: ChunkStoreHandle,
    registration_time: jiff::Timestamp,
}

pub struct Dataset {
    id: EntryId,
    name: String,
    partitions: HashMap<PartitionId, Partition>,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),
            kind: EntryKind::Dataset,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn as_dataset_entry(&self) -> DatasetEntry {
        DatasetEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Dataset,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            dataset_details: Default::default(),

            handle: DatasetHandle {
                id: Some(self.id),
                store_kind: StoreKind::Recording,
                url: url::Url::parse(&format!("memory:///{}", self.id)).expect("valid url"),
            },
        }
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let schemas = self.partitions.values().map(|partition| {
            let columns = partition.store_handle.read().schema();
            let fields = columns.arrow_fields();
            Schema::new_with_metadata(fields, HashMap::default())
        });

        Schema::try_merge(schemas)
    }

    pub fn partition_ids(&self) -> impl Iterator<Item = PartitionId> {
        self.partitions.keys().cloned()
    }

    pub fn partition_table(&self) -> arrow::error::Result<RecordBatch> {
        let (partition_ids, last_updated_at, num_chunks, size_bytes): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = itertools::multiunzip(self.partitions.iter().map(|(store_id, partition)| {
            let store = partition.store_handle.read();
            let size_bytes: u64 = store
                .iter_chunks()
                .map(|chunk| chunk.heap_size_bytes())
                .sum();

            (
                store_id.to_string(),
                partition.registration_time.as_nanosecond() as i64,
                store.num_chunks() as u64,
                size_bytes,
            )
        }));

        let layers = vec![vec!["base".to_owned()]; partition_ids.len()];

        let storage_urls = partition_ids
            .iter()
            .map(|partition_id| vec![format!("memory:///{}/{partition_id}", self.id)])
            .collect();

        ScanPartitionTableResponse::create_dataframe(
            partition_ids,
            layers,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
    }

    pub fn partition_store_handle(&self, partition_id: &PartitionId) -> Option<&ChunkStoreHandle> {
        self.partitions.get(partition_id).map(|p| &p.store_handle)
    }

    pub fn add_partition(&mut self, partition_id: PartitionId, store_handle: ChunkStoreHandle) {
        re_log::debug!(?partition_id, "add_partition");
        self.partitions.insert(
            partition_id,
            Partition {
                store_handle,
                registration_time: jiff::Timestamp::now(),
            },
        );
        self.updated_at = jiff::Timestamp::now();
    }

    pub fn load_rrd(
        &mut self,
        path: &Path,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<BTreeSet<PartitionId>, Error> {
        re_log::info!("Loading RRD: {}", path.display());
        let contents =
            ChunkStore::handle_from_rrd_filepath(&ChunkStoreConfig::CHANGELOG_DISABLED, path)
                .map_err(Error::RrdLoadingError)?;

        let mut new_partition_ids = BTreeSet::default();

        for (store_id, chunk_store) in contents {
            if !store_id.is_recording() {
                continue;
            }

            let partition_id = PartitionId::new(store_id.recording_id().to_string());

            match self.partitions.entry(partition_id.clone()) {
                Entry::Vacant(entry) => {
                    new_partition_ids.insert(partition_id);
                    entry.insert(Partition {
                        store_handle: chunk_store,
                        registration_time: jiff::Timestamp::now(),
                    });
                }
                Entry::Occupied(mut entry) => match on_duplicate {
                    IfDuplicateBehavior::Overwrite => {
                        re_log::info!("Overwriting {partition_id}");
                        entry.insert(Partition {
                            store_handle: chunk_store,
                            registration_time: jiff::Timestamp::now(),
                        });
                    }
                    IfDuplicateBehavior::Skip => {
                        re_log::info!("Ignoring {partition_id}: it already exists");
                    }
                    IfDuplicateBehavior::Error => {
                        return Err(Error::DuplicateEntryNameError(partition_id.to_string()));
                    }
                },
            }
        }

        Ok(new_partition_ids)
    }
}

#[derive(Clone)]
pub struct Table {
    id: EntryId,
    name: String,
    provider: Arc<dyn TableProvider>,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,

    system_table: Option<SystemTable>,
}

impl Table {
    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),
            kind: EntryKind::Table,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn provider(&self) -> &Arc<dyn TableProvider> {
        &self.provider
    }

    pub fn as_table_entry(&self) -> TableEntry {
        let provider_details = match &self.system_table {
            Some(s) => s.try_as_any().expect("system_table should always be valid"),
            None => Default::default(),
        };

        TableEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Table,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },

            provider_details,
        }
    }
}

pub struct InMemoryStore {
    // TODO(ab): track created/modified time
    datasets: HashMap<EntryId, Dataset>,
    tables: HashMap<EntryId, Table>,
    id_by_name: HashMap<String, EntryId>,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        let mut ret = Self {
            tables: HashMap::default(),
            datasets: HashMap::default(),
            id_by_name: HashMap::default(),
        };
        ret.update_entries_table()
            .expect("update_entries_table should never fail on initialization.");
        ret
    }
}

impl InMemoryStore {
    /// Load a directory of RRDs.
    pub fn load_directory_as_dataset(
        &mut self,
        named_path: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        let directory = named_path.path.canonicalize()?;
        if !directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Expected a directory, got: {}", directory.display()),
            )
            .into());
        }

        let entry_name = match &named_path.name {
            Some(name) => name.into(),
            None => directory
                .file_name()
                .expect("the directory should have a name and the path was canonicalized")
                .to_string_lossy(),
        };

        let dataset = self
            .create_dataset(&entry_name)
            .expect("Name cannot yet exist");

        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let is_rrd = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|s| s.to_lowercase().ends_with(".rrd"));

                if is_rrd {
                    dataset.load_rrd(&entry.path(), on_duplicate)?;
                }
            }
        }

        self.update_entries_table()?;
        Ok(())
    }

    pub async fn load_directory_as_table(
        &mut self,
        named_path: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        let directory = named_path.path.canonicalize()?;
        if !directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Expected a directory, got: {}", directory.display()),
            )
            .into());
        }

        let entry_name = match &named_path.name {
            Some(name) => name.into(),
            None => directory
                .file_name()
                .expect("the directory should have a name and the path was canonicalized")
                .to_string_lossy(),
        };

        // Verify it is a valid lance table
        let path = directory.to_str().ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Expected a valid path, got: {}", directory.display()),
        ))?;

        let table = lance::Dataset::open(path)
            .await
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        let provider = Arc::new(LanceTableProvider::new(Arc::new(table), false, false));

        let entry_id = EntryId::new();

        match self.table_by_name(entry_name.as_ref()) {
            None => {
                self.add_table_entry(entry_name.as_ref(), entry_id, provider)?;
            }
            Some(_) => match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    re_log::info!("Overwriting {entry_name}");
                    self.add_table_entry(entry_name.as_ref(), entry_id, provider)?;
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring {entry_name}: it already exists");
                }
                IfDuplicateBehavior::Error => {
                    return Err(Error::DuplicateEntryNameError(entry_name.to_string()));
                }
            },
        }

        Ok(())
    }

    fn add_table_entry(
        &mut self,
        entry_name: &str,
        entry_id: EntryId,
        provider: Arc<dyn TableProvider>,
    ) -> Result<(), Error> {
        self.id_by_name.insert(entry_name.to_owned(), entry_id);
        self.tables.insert(
            entry_id,
            Table {
                id: entry_id,
                name: entry_name.to_owned(),
                provider,
                created_at: jiff::Timestamp::now(),
                updated_at: jiff::Timestamp::now(),
                system_table: None,
            },
        );

        self.update_entries_table()
    }

    /// Update the table of entries. This method must be called after
    /// any changes to either the registered datasets or tables. We
    /// can remove this restriction if we change the store to be an
    /// `Arc<Mutex<_>>` and then have an ac-hoc table generation.
    /// TODO(#11369)
    fn update_entries_table(&mut self) -> Result<(), Error> {
        let entries_table_id = *self
            .id_by_name
            .entry(ENTRIES_TABLE_NAME.to_owned())
            .or_insert(EntryId::new());
        let prior_entries_table = self.tables.remove(&entries_table_id);

        let entries_table = Arc::new(self.entries_table()?);
        self.tables.insert(
            entries_table_id,
            Table {
                id: entries_table_id,
                name: ENTRIES_TABLE_NAME.to_owned(),
                provider: entries_table,
                created_at: prior_entries_table
                    .map(|t| t.created_at)
                    .unwrap_or(Timestamp::now()),
                updated_at: Timestamp::now(),
                system_table: Some(SystemTable {
                    kind: SystemTableKind::Entries,
                }),
            },
        );

        Ok(())
    }

    pub fn create_dataset(&mut self, name: &str) -> Result<&mut Dataset, Error> {
        re_log::debug!(name, "create_dataset");
        let name = name.to_owned();
        if self.id_by_name.contains_key(&name) {
            return Err(Error::DuplicateEntryNameError(name));
        }

        let entry_id = EntryId::new();
        self.id_by_name.insert(name.clone(), entry_id);

        Ok(self.datasets.entry(entry_id).or_insert_with(|| Dataset {
            id: entry_id,
            name,
            partitions: HashMap::new(),
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        }))
    }

    pub fn delete_dataset(&mut self, entry_id: EntryId) -> Result<(), Error> {
        re_log::debug!(?entry_id, "delete_dataset");
        if let Some(dataset) = self.datasets.remove(&entry_id) {
            self.id_by_name.remove(&dataset.name);
            Ok(())
        } else {
            Err(Error::EntryIdNotFound(entry_id))
        }
    }

    pub fn dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.datasets.get(&entry_id)
    }

    pub fn dataset_mut(&mut self, entry_id: EntryId) -> Option<&mut Dataset> {
        self.datasets.get_mut(&entry_id)
    }

    pub fn dataset_by_name(&self, name: &str) -> Option<&Dataset> {
        let entry_id = self.id_by_name.get(name).copied()?;
        self.dataset(entry_id)
    }

    pub fn iter_datasets(&self) -> impl Iterator<Item = &Dataset> {
        self.datasets.values()
    }

    pub fn table(&self, entry_id: EntryId) -> Option<&Table> {
        self.tables.get(&entry_id)
    }

    pub fn table_mut(&mut self, entry_id: EntryId) -> Option<&mut Table> {
        self.tables.get_mut(&entry_id)
    }

    pub fn table_by_name(&self, name: &str) -> Option<&Table> {
        let entry_id = self.id_by_name.get(name).copied()?;
        self.table(entry_id)
    }

    pub fn iter_tables(&self) -> impl Iterator<Item = &Table> {
        self.tables.values()
    }
}

fn generate_entries_table(entries: &[EntryDetails]) -> Result<RecordBatch, Error> {
    #[allow(clippy::type_complexity)]
    let (id, name, entry_kind, created_at, updated_at): (
        Vec<Tuid>,
        Vec<String>,
        Vec<i32>,
        Vec<i64>,
        Vec<i64>,
    ) = entries
        .iter()
        .map(|entry| {
            (
                entry.id.id,
                entry.name.clone(),
                entry.kind as i32,
                entry.created_at.as_nanosecond() as i64,
                entry.updated_at.as_nanosecond() as i64,
            )
        })
        .multiunzip();

    let id_arr = id
        .to_arrow()
        .map_err(|err| DataFusionError::External(Box::new(err)))?;
    let name_arr = Arc::new(StringArray::from(name)) as ArrayRef;
    let kind_arr = Arc::new(Int32Array::from(entry_kind)) as ArrayRef;
    let created_at_arr = Arc::new(TimestampNanosecondArray::from(created_at)) as ArrayRef;
    let updated_at_arr = Arc::new(TimestampNanosecondArray::from(updated_at)) as ArrayRef;

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", Tuid::arrow_datatype(), false),
            Field::new("name", DataType::Utf8, false),
            Field::new("entry_kind", DataType::Int32, false),
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "updated_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
        ],
        HashMap::new(),
    ));

    let num_rows = id_arr.len();
    let rb = RecordBatch::try_new_with_options(
        schema,
        vec![id_arr, name_arr, kind_arr, created_at_arr, updated_at_arr],
        &RecordBatchOptions::default().with_row_count(Some(num_rows)),
    )
    .map_err(DataFusionError::from)?;

    Ok(rb)
}

// Generate both functions
impl InMemoryStore {
    fn dataset_entries_table(&self) -> Result<RecordBatch, Error> {
        let details = self
            .datasets
            .values()
            .map(|dataset| dataset.as_entry_details())
            .collect::<Vec<_>>();
        generate_entries_table(&details)
    }

    fn table_entries_table(&self) -> Result<RecordBatch, Error> {
        let details = self
            .tables
            .values()
            .map(|dataset| dataset.as_entry_details())
            .collect::<Vec<_>>();
        generate_entries_table(&details)
    }

    pub fn entries_table(&self) -> Result<MemTable, Error> {
        let dataset_rb = self.dataset_entries_table()?;
        let table_rb = self.table_entries_table()?;
        let schema = dataset_rb.schema();

        let result_table = MemTable::try_new(schema, vec![vec![dataset_rb, table_rb]])?;

        Ok(result_table)
    }
}
