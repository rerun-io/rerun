use std::sync::Arc;

use ahash::HashMap;
use arrow::array::{
    ArrayRef, Int32Array, RecordBatch, RecordBatchOptions, StringArray, TimestampNanosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::catalog::MemTable;
use datafusion::common::DataFusionError;
use itertools::Itertools as _;
use re_chunk_store::{Chunk, ChunkStoreConfig};
use re_log_types::{EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::EntryKind;
use re_protos::cloud::v1alpha1::ext::{DatasetDetails, EntryDetails, ProviderDetails, TableEntry};
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_tuid::Tuid;
use re_types_core::{ComponentBatch as _, Loggable as _};

use crate::OnError;
use crate::entrypoint::NamedPath;
use crate::store::table::TableType;
use crate::store::task_registry::TaskRegistry;
use crate::store::{ChunkKey, Dataset, Error, Table};

const ENTRIES_TABLE_NAME: &str = "__entries";

pub struct InMemoryStore {
    datasets: HashMap<EntryId, Dataset>,
    tables: HashMap<EntryId, Table>,
    id_by_name: HashMap<String, EntryId>,
    task_registry: TaskRegistry,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        let mut ret = Self {
            tables: HashMap::default(),
            datasets: HashMap::default(),
            id_by_name: HashMap::default(),
            task_registry: TaskRegistry::default(),
        };
        ret.update_entries_table()
            .expect("update_entries_table should never fail on initialization.");
        ret
    }
}

impl InMemoryStore {
    pub fn chunk_store_config() -> re_chunk_store::ChunkStoreConfig {
        ChunkStoreConfig::CHANGELOG_DISABLED
            .apply_env()
            .unwrap_or(ChunkStoreConfig::CHANGELOG_DISABLED)
    }

    /// Returns the chunks corresponding to the provided chunk keys.
    ///
    /// Important: there is no guarantee on the order of the returned chunks.
    pub fn chunks_from_chunk_keys(
        &self,
        chunk_keys: &[ChunkKey],
    ) -> Result<Vec<(StoreId, Arc<Chunk>)>, Error> {
        // sort keys per dataset, segment, layer
        let mut chunk_key_index: HashMap<
            &EntryId,
            HashMap<&SegmentId, HashMap<&str, Vec<&ChunkKey>>>,
        > = Default::default();

        for chunk_key in chunk_keys {
            chunk_key_index
                .entry(&chunk_key.dataset_id)
                .or_default()
                .entry(&chunk_key.segment_id)
                .or_default()
                .entry(&chunk_key.layer_name)
                .or_default()
                .push(chunk_key);
        }

        let mut result = Vec::with_capacity(chunk_keys.len());

        for (dataset_id, segment_index) in chunk_key_index {
            let dataset = self.dataset(*dataset_id)?;

            for (segment_id, layer_index) in segment_index {
                let segment = dataset.segment(segment_id)?;

                let store_id = StoreId::new(
                    StoreKind::Recording,
                    dataset_id.to_string(),
                    segment_id.id.as_str(),
                );

                for (layer_name, chunk_keys) in layer_index {
                    let store_handle = segment
                        .layer(layer_name)
                        .ok_or_else(|| Error::LayerNameNotFound {
                            layer_name: layer_name.to_owned(),
                            segment_id: segment_id.clone(),
                            entry_id: *dataset_id,
                        })?
                        .store_handle()
                        .read();

                    for chunk_key in chunk_keys {
                        let chunk = store_handle
                            .physical_chunk(&chunk_key.chunk_id)
                            .ok_or_else(|| Error::ChunkNotFound(chunk_key.clone()))?;

                        result.push((store_id.clone(), Arc::clone(chunk)));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Load a directory of RRDs.
    //TODO(ab): maybe we could be smart with .rbl and auto-setup a blueprint dataset?
    pub async fn load_directory_as_dataset(
        &mut self,
        named_path: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
        on_error: OnError,
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
            .create_dataset(entry_name.into(), None)
            .expect("Name cannot yet exist");

        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let is_rrd = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|s| s.to_lowercase().ends_with(".rrd"));

                if is_rrd
                    && let Err(err) = dataset
                        .load_rrd(&entry.path(), None, on_duplicate, StoreKind::Recording)
                        .await
                {
                    match on_error {
                        OnError::Continue => {
                            re_log::warn!("Failed loading file in {}: {err}", directory.display());
                        }
                        OnError::Abort => {
                            return Err(err);
                        }
                    }
                }
            }
        }

        self.update_entries_table()?;

        re_log::info!("Finished loading {}", directory.display());

        Ok(())
    }

    #[cfg(feature = "lance")]
    pub async fn load_directory_as_table(
        &mut self,
        named_path: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<EntryId, Error> {
        use std::sync::Arc;

        use re_protos::cloud::v1alpha1::ext::LanceTable;

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
        let path = directory.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Expected a valid path, got: {}", directory.display()),
            )
        })?;

        let table = TableType::LanceDataset(Arc::new(
            lance::Dataset::open(path)
                .await
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?,
        ));
        let table_url = url::Url::from_directory_path(&directory).map_err(|_err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot turn directory into URL",
            )
        })?;

        let entry_id = EntryId::new();
        let provider_details = LanceTable { table_url };

        match self.table_by_name(entry_name.as_ref()) {
            None => {
                self.add_table_entry(entry_name.as_ref(), entry_id, table, provider_details)?;
            }
            Some(_) => match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    re_log::info!("Overwriting {entry_name}");
                    self.add_table_entry(entry_name.as_ref(), entry_id, table, provider_details)?;
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring {entry_name}: it already exists");
                }
                IfDuplicateBehavior::Error => {
                    return Err(Error::DuplicateEntryNameError(entry_name.to_string()));
                }
            },
        }

        Ok(entry_id)
    }

    pub fn rename_entry(&mut self, entry_id: EntryId, entry_name: String) -> Result<(), Error> {
        if let Some(existing_entry_id) = self.id_by_name.get(&entry_name) {
            return if existing_entry_id == &entry_id {
                // nothing to do, the rename is a no-op
                Ok(())
            } else {
                // name is already taken
                Err(Error::DuplicateEntryNameError(entry_name))
            };
        }

        if let Some(dataset) = self.datasets.get_mut(&entry_id) {
            dataset.set_name(entry_name.clone());
        } else if let Some(table) = self.tables.get_mut(&entry_id) {
            table.set_name(entry_name.clone());
        } else {
            return Err(Error::EntryIdNotFound(entry_id));
        }

        self.id_by_name.insert(entry_name, entry_id);
        self.update_entries_table()
    }

    pub fn entry_details(&self, entry_id: EntryId) -> Result<EntryDetails, Error> {
        if let Some(dataset) = self.datasets.get(&entry_id) {
            Ok(dataset.as_entry_details())
        } else if let Some(table) = self.tables.get(&entry_id) {
            Ok(table.as_entry_details())
        } else {
            Err(Error::EntryIdNotFound(entry_id))
        }
    }

    #[cfg(feature = "lance")] // only used by the `lance` feature
    fn add_table_entry(
        &mut self,
        entry_name: &str,
        entry_id: EntryId,
        table: TableType,
        provider_details: re_protos::cloud::v1alpha1::ext::LanceTable,
    ) -> Result<(), Error> {
        self.id_by_name.insert(entry_name.to_owned(), entry_id);
        self.tables.insert(
            entry_id,
            Table::new(
                entry_id,
                entry_name.to_owned(),
                table,
                None,
                ProviderDetails::LanceTable(provider_details),
            ),
        );

        self.update_entries_table()
    }

    /// Create a (regular) dataset with a matching blueprint dataset.
    ///
    /// The server is typically responsible for setting the dataset id, so use `Some` at your own
    /// risk for `dataset_id`.
    pub fn create_dataset(
        &mut self,
        dataset_name: String,
        dataset_id: Option<EntryId>,
    ) -> Result<&mut Dataset, Error> {
        let dataset_id = dataset_id.unwrap_or_else(EntryId::new);
        let blueprint_dataset_id = EntryId::new();
        let blueprint_dataset_name = format!("__bp_{dataset_id}");

        self.create_dataset_impl(
            blueprint_dataset_name,
            blueprint_dataset_id,
            StoreKind::Blueprint,
            None,
        )?;

        let dataset_details = DatasetDetails {
            blueprint_dataset: Some(blueprint_dataset_id),
            default_blueprint_segment: None,
        };

        self.create_dataset_impl(
            dataset_name,
            dataset_id,
            StoreKind::Recording,
            Some(dataset_details),
        )
    }

    /// Create a dataset of the given kind with the given details.
    fn create_dataset_impl(
        &mut self,
        name: String,
        entry_id: EntryId,
        store_kind: StoreKind,
        details: Option<DatasetDetails>,
    ) -> Result<&mut Dataset, Error> {
        re_log::debug!(name, "create_dataset");
        if self.id_by_name.contains_key(&name) {
            return Err(Error::DuplicateEntryNameError(name));
        }

        if self.id_exists(&entry_id) {
            return Err(Error::DuplicateEntryIdError(entry_id));
        }

        self.id_by_name.insert(name.clone(), entry_id);

        self.datasets.insert(
            entry_id,
            Dataset::new(entry_id, name, store_kind, details.unwrap_or_default()),
        );

        self.update_entries_table()?;
        self.dataset_mut(entry_id)
    }

    /// Delete the provided entry.
    ///
    /// For dataset, the corresponding blueprint dataset will be deleted as well.
    pub fn delete_entry(&mut self, entry_id: EntryId) -> Result<(), Error> {
        re_log::debug!(?entry_id, "delete_entry");

        if let Some(table) = self.tables.remove(&entry_id) {
            self.id_by_name.remove(table.name());
            self.update_entries_table()?;
            Ok(())
        } else if let Some(dataset) = self.datasets.remove(&entry_id) {
            self.id_by_name.remove(dataset.name());
            self.update_entries_table()?;

            if let Some(blueprint_entry_id) = dataset.dataset_details().blueprint_dataset {
                self.delete_entry(blueprint_entry_id)
            } else {
                Ok(())
            }
        } else {
            Err(Error::EntryIdNotFound(entry_id))
        }
    }

    /// Update the table of entries. This method must be called after
    /// any changes to either the registered datasets or tables. We
    /// can remove this restriction if we change the store to be an
    /// `Arc<Mutex<_>>` and then have an ac-hoc table generation.
    /// TODO(#11369)
    fn update_entries_table(&mut self) -> Result<(), Error> {
        use std::sync::Arc;

        use re_protos::cloud::v1alpha1::SystemTableKind;
        use re_protos::cloud::v1alpha1::ext::SystemTable;

        let entries_table_id = *self
            .id_by_name
            .entry(ENTRIES_TABLE_NAME.to_owned())
            .or_insert_with(EntryId::new);
        let prior_entries_table = self.tables.remove(&entries_table_id);

        let entries_table = Arc::new(self.entries_table()?);
        self.tables.insert(
            entries_table_id,
            Table::new(
                entries_table_id,
                ENTRIES_TABLE_NAME.to_owned(),
                TableType::DataFusionTable(entries_table),
                prior_entries_table.map(|t| t.created_at()),
                ProviderDetails::SystemTable(SystemTable {
                    kind: SystemTableKind::Entries,
                }),
            ),
        );

        Ok(())
    }

    pub fn dataset(&self, entry_id: EntryId) -> Result<&Dataset, Error> {
        self.datasets
            .get(&entry_id)
            .ok_or(Error::EntryIdNotFound(entry_id))
    }

    pub fn dataset_mut(&mut self, entry_id: EntryId) -> Result<&mut Dataset, Error> {
        self.datasets
            .get_mut(&entry_id)
            .ok_or(Error::EntryIdNotFound(entry_id))
    }

    pub fn dataset_by_name(&self, name: &str) -> Result<&Dataset, Error> {
        let entry_id = self
            .id_by_name
            .get(name)
            .copied()
            .ok_or(Error::EntryNameNotFound(name.to_owned()))?;
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

    pub fn id_by_name(&self, name: &str) -> Option<&EntryId> {
        self.id_by_name.get(name)
    }

    pub fn id_exists(&self, id: &EntryId) -> bool {
        self.tables.contains_key(id) || self.datasets.contains_key(id)
    }

    pub fn task_registry(&self) -> &TaskRegistry {
        &self.task_registry
    }

    pub async fn create_table_entry(
        &mut self,
        name: &str,
        url: &url::Url,
        schema: SchemaRef,
    ) -> Result<TableEntry, Error> {
        re_log::debug!(name, "create_table");
        if self.id_by_name.contains_key(name) {
            return Err(Error::DuplicateEntryNameError(name.to_owned()));
        }

        let entry_id = EntryId::new();

        let table = Table::create_table_entry(entry_id, name, url, schema).await?;
        let table_entry = table.as_table_entry();

        self.id_by_name.insert(name.to_owned(), entry_id);
        self.tables.insert(entry_id, table);
        self.update_entries_table()?;

        Ok(table_entry)
    }
}

fn generate_entries_table(entries: &[EntryDetails]) -> Result<RecordBatch, Error> {
    #[expect(clippy::type_complexity)]
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
        Default::default(),
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

        // TODO(#11369): this is a hack to have the entries table until we use a proper table-
        // provider-based approach. When we do, we must seed the `__entries` table in the in-memory
        // store upon initialization.
        let entry_table_rb = generate_entries_table(&[EntryDetails {
            id: EntryId::from(Tuid::from_bytes([0; 16])),
            name: ENTRIES_TABLE_NAME.to_owned(),
            kind: EntryKind::Table,
            created_at: Default::default(),
            updated_at: Default::default(),
        }])?;

        let schema = dataset_rb.schema();

        let result_table =
            MemTable::try_new(schema, vec![vec![dataset_rb, table_rb, entry_table_rb]])?;

        Ok(result_table)
    }
}
