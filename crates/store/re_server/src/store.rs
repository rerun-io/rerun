use arrow::{array::RecordBatch, datatypes::Schema};
use datafusion::catalog::TableProvider;
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{EntryId, StoreKind};
use re_protos::{
    cloud::v1alpha1::ScanPartitionTableResponse,
    cloud::v1alpha1::{
        EntryKind,
        ext::{DatasetEntry, EntryDetails},
    },
    common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, PartitionId},
};
use std::sync::Arc;
use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    fs::File,
    path::Path,
};
use arrow::array::TimestampNanosecondArray;

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
        }
    }
}

pub struct Partition {
    entity_db: EntityDb,
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
            let columns = partition.entity_db.storage_engine().store().schema();
            let fields = columns.arrow_fields();
            Schema::new_with_metadata(fields, HashMap::default())
        });

        Schema::try_merge(schemas)
    }

    pub fn partition_ids(&self) -> impl Iterator<Item = PartitionId> {
        self.partitions.keys().cloned()
    }

    pub fn partition_table(&self) -> arrow::error::Result<RecordBatch> {
        let (partition_ids, registration_times): (Vec<_>, Vec<_>) = self
            .partitions
            .iter()
            .map(|(store_id, partition)| {
                (
                    store_id.to_string(),
                    partition.registration_time.as_nanosecond() as i64,
                )
            })
            .unzip();

        let partition_types = vec!["rrd".to_owned(); partition_ids.len()];

        let storage_urls = partition_ids
            .iter()
            .map(|partition_id| format!("memory:///{}/{partition_id}", self.id))
            .collect();

        let partition_manifest_updated_ats = vec![None; partition_ids.len()];
        let partition_manifest_urls = vec![None; partition_ids.len()];

        ScanPartitionTableResponse::create_dataframe(
            partition_ids,
            partition_types,
            storage_urls,
            registration_times,
            partition_manifest_updated_ats,
            partition_manifest_urls,
        )
    }

    pub fn partition(&self, partition_id: &PartitionId) -> Option<&EntityDb> {
        self.partitions.get(partition_id).map(|p| &p.entity_db)
    }

    pub fn add_partition(&mut self, partition_id: PartitionId, entity_db: EntityDb) {
        re_log::debug!(?partition_id, "add_partition");
        self.partitions.insert(
            partition_id,
            Partition {
                entity_db,
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
        let mut contents = StoreBundle::from_rrd(File::open(path)?)?;

        let mut new_partition_ids = BTreeSet::default();

        for entity_db in contents.drain_entity_dbs() {
            let store_id = entity_db.store_id();
            if !store_id.is_recording() {
                continue;
            }

            let partition_id = PartitionId::new(store_id.recording_id().to_string());

            match self.partitions.entry(partition_id.clone()) {
                Entry::Vacant(entry) => {
                    new_partition_ids.insert(partition_id);
                    entry.insert(Partition {
                        entity_db,
                        registration_time: jiff::Timestamp::now(),
                    });
                }
                Entry::Occupied(mut entry) => match on_duplicate {
                    IfDuplicateBehavior::Overwrite => {
                        re_log::info!("Overwriting {partition_id}");
                        entry.insert(Partition {
                            entity_db,
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

pub struct Table {
    id: EntryId,
    name: String,
    // pub(crate) provider: Arc<dyn TableProvider>,
    url: String,

    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,
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

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[derive(Default)]
pub struct InMemoryStore {
    // TODO(ab): track created/modified time
    datasets: HashMap<EntryId, Dataset>,
    tables: HashMap<EntryId, Table>,
    id_by_name: HashMap<String, EntryId>,
}

impl InMemoryStore {
    /// Load a directory of RRDs.
    pub fn load_directory_as_dataset(
        &mut self,
        directory: &std::path::Path,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        let directory = directory.canonicalize()?;
        if !directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Expected a directory, got: {}", directory.display()),
            )
            .into());
        }

        let entry_name = directory
            .file_name()
            .expect("the directory should have a name and the path was canonicalized")
            .to_string_lossy();

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

        Ok(())
    }

    pub async fn load_directory_as_table(
        &mut self,
        directory: &std::path::Path,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        let directory = directory.canonicalize()?;
        if !directory.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Expected a directory, got: {}", directory.display()),
            )
                .into());
        }

        let entry_name = directory
            .file_name()
            .expect("the directory should have a name and the path was canonicalized")
            .to_string_lossy();

        // Verify it is a valid lance table
        let path = directory.to_str().ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Expected a valid path, got: {}", directory.display()),
        ))?;

        let table = lance::Dataset::open(path)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let entry_id = EntryId::new();

        match self.table_by_name(entry_name.as_ref()) {
            None => {
                self.id_by_name.insert(entry_name.to_string(), entry_id);
                self.tables.insert(entry_id.clone(), Table {
                    id: entry_id,
                    name: entry_name.to_string(),
                    url: format!("file://{path}"),
                    created_at: jiff::Timestamp::now(),
                    updated_at: jiff::Timestamp::now(),
                });
            }
            Some(_) => match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    re_log::info!("Overwriting {entry_name}");

                    self.id_by_name.insert(entry_name.to_string(), entry_id);
                    self.tables.insert(entry_id.clone(), Table {
                        id: entry_id,
                        name: entry_name.to_string(),
                        url: format!("file://{path}"),
                        created_at: jiff::Timestamp::now(),
                        updated_at: jiff::Timestamp::now(),
                    });
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
