use std::collections::HashMap;
use std::fs::File;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;

use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{EntryId, StoreKind};
use re_protos::catalog::v1alpha1::EntryKind;
use re_protos::catalog::v1alpha1::ext::{DatasetEntry, EntryDetails};
use re_protos::common::v1alpha1::ext::{DatasetHandle, PartitionId};
use re_protos::manifest_registry::v1alpha1::ScanPartitionTableResponse;

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
            Error::IoError(err) => Self::internal(format!("IO error: {err}")),
            Error::StoreLoadError(err) => Self::internal(format!("Store load error: {err}")),
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
    //TODO:
    //storage url
}

impl Dataset {
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

            handle: DatasetHandle {
                id: Some(self.id),
                //TODO
                url: url::Url::parse("file:///tmp/unsupported").expect("valid url"),
            },
        }
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let schemas = self.partitions.values().map(|partition| {
            let columns = partition.entity_db.storage_engine().store().schema();
            let fields = columns.arrow_fields();
            Schema::new(fields)
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

        //TODO
        let storage_urls = vec!["file:///tmp/unsupported".to_owned(); partition_ids.len()];

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

    pub fn partition(&self, partition_id: PartitionId) -> Option<&EntityDb> {
        self.partitions.get(&partition_id).map(|p| &p.entity_db)
    }

    pub fn add_partition(&mut self, partition_id: PartitionId, entity_db: EntityDb) {
        self.partitions.insert(partition_id, Partition {
            entity_db,
            registration_time: jiff::Timestamp::now(),
        });
        self.updated_at = jiff::Timestamp::now();
    }
}

#[derive(Default)]
pub struct InMemoryStore {
    //TODO(ab): track created/modified time
    datasets: HashMap<EntryId, Dataset>,
    id_by_name: HashMap<String, EntryId>,
}

impl InMemoryStore {
    /// Load a directory of RRDs.
    pub fn load_directory_as_dataset(&mut self, directory: &std::path::Path) -> Result<(), Error> {
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

        let entry_id = self
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
                    re_log::info!("Loading RRD: {}", entry.path().display());

                    let mut contents = StoreBundle::from_rrd(File::open(entry.path())?)?;

                    for entity_db in contents.drain_entity_dbs() {
                        let store_id = entity_db.store_id();

                        if store_id.kind == StoreKind::Recording {
                            self.datasets
                                .entry(entry_id)
                                .or_insert_with(|| Dataset {
                                    id: entry_id,
                                    name: entry_name.to_string(),
                                    partitions: HashMap::new(),
                                    created_at: jiff::Timestamp::now(),
                                    updated_at: jiff::Timestamp::now(),
                                })
                                .partitions
                                .insert(PartitionId::new((*store_id.id).clone()), Partition {
                                    entity_db,
                                    registration_time: jiff::Timestamp::now(),
                                });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn create_dataset(&mut self, name: &str) -> Result<EntryId, Error> {
        let name = name.to_owned();
        if self.id_by_name.contains_key(&name) {
            return Err(Error::DuplicateEntryNameError(name));
        }

        let entry_id = EntryId::new();
        self.id_by_name.insert(name.clone(), entry_id);

        self.datasets.insert(entry_id, Dataset {
            id: entry_id,
            name,
            partitions: HashMap::new(),
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        });

        Ok(entry_id)
    }

    pub fn delete_dataset(&mut self, entry_id: EntryId) -> Result<(), Error> {
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

    pub fn iter_datasets(&self) -> impl Iterator<Item = &Dataset> {
        self.datasets.values()
    }
}
