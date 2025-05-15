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

    #[error("Entry name '{0}'already exists")]
    DuplicateEntryNameError(String),
}

pub struct Dataset {
    id: EntryId,
    name: String,
    partitions: HashMap<PartitionId, EntityDb>,
    //TODO
    // storage url
    // created/modified time
}

impl Dataset {
    pub fn as_entry_details(&self) -> EntryDetails {
        EntryDetails {
            id: self.id,
            name: self.name.clone(),

            kind: EntryKind::Dataset,
            //TODO
            created_at: jiff::Timestamp::default(),
            updated_at: jiff::Timestamp::default(),
        }
    }

    pub fn as_dataset_entry(&self) -> DatasetEntry {
        DatasetEntry {
            details: EntryDetails {
                id: self.id,
                name: self.name.clone(),
                kind: EntryKind::Dataset,

                //TODO
                created_at: jiff::Timestamp::default(),
                updated_at: jiff::Timestamp::default(),
            },

            handle: DatasetHandle {
                id: Some(self.id),
                //TODO
                url: url::Url::parse("file:///tmp/unsupported").expect("valid url"),
            },
        }
    }

    pub fn schema(&self) -> arrow::error::Result<Schema> {
        let schemas = self.partitions.values().map(|entity_db| {
            let columns = entity_db.storage_engine().store().schema();
            let fields = columns.arrow_fields();
            Schema::new(fields)
        });

        Schema::try_merge(schemas)
    }

    pub fn partition_ids(&self) -> impl Iterator<Item = PartitionId> {
        self.partitions.keys().cloned()
    }

    pub fn partition_table(&self) -> arrow::error::Result<RecordBatch> {
        let partition_ids = self
            .partitions
            .keys()
            .map(|store_id| store_id.to_string())
            .collect::<Vec<_>>();

        let partition_types = vec!["rrd".to_owned(); partition_ids.len()];

        //TODO
        let storage_urls = vec!["file:///tmp/unsupported".to_owned(); partition_ids.len()];
        let registration_times = vec![jiff::Timestamp::default().as_second(); partition_ids.len()];

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
        self.partitions.get(&partition_id)
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
                                })
                                .partitions
                                .insert(PartitionId::new((*store_id.id).clone()), entity_db);
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

        Ok(entry_id)
    }

    pub fn dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.datasets.get(&entry_id)
    }

    pub fn iter_datasets(&self) -> impl Iterator<Item = &Dataset> {
        self.datasets.values()
    }
}
