use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, Int32Array, RecordBatch, RecordBatchOptions, StringArray, TimestampNanosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use datafusion::catalog::{MemTable, TableProvider};
use datafusion::common::DataFusionError;
use itertools::Itertools as _;
use lance::datafusion::LanceTableProvider;

use re_chunk_store::ChunkStoreConfig;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::EntryKind;
use re_protos::{
    cloud::v1alpha1::{
        SystemTableKind,
        ext::{EntryDetails, SystemTable},
    },
    common::v1alpha1::ext::IfDuplicateBehavior,
};
use re_tuid::Tuid;
use re_types_core::{ComponentBatch as _, Loggable as _};

use crate::entrypoint::NamedPath;
use crate::store::{Dataset, Error, Table};

const ENTRIES_TABLE_NAME: &str = "__entries";

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
    pub fn chunk_store_config() -> re_chunk_store::ChunkStoreConfig {
        ChunkStoreConfig::CHANGELOG_DISABLED
            .apply_env()
            .unwrap_or(ChunkStoreConfig::CHANGELOG_DISABLED)
    }

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
                    dataset.load_rrd(&entry.path(), None, on_duplicate)?;
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
            Table::new(entry_id, entry_name.to_owned(), provider, None, None),
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
            Table::new(
                entries_table_id,
                ENTRIES_TABLE_NAME.to_owned(),
                entries_table,
                prior_entries_table.map(|t| t.created_at()),
                Some(SystemTable {
                    kind: SystemTableKind::Entries,
                }),
            ),
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

        Ok(self
            .datasets
            .entry(entry_id)
            .or_insert_with(|| Dataset::new(entry_id, name)))
    }

    pub fn delete_dataset(&mut self, entry_id: EntryId) -> Result<(), Error> {
        re_log::debug!(?entry_id, "delete_dataset");
        if let Some(dataset) = self.datasets.remove(&entry_id) {
            self.id_by_name.remove(dataset.name());
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
