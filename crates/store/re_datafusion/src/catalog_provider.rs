use crate::TableEntryTableProvider;
use ahash::{HashMap, HashSet};
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, TableProvider};
use datafusion::common::{DataFusionError, Result as DataFusionResult, TableReference, exec_err};
use parking_lot::Mutex;
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_redap_client::ConnectionClient;
use std::any::Any;
use std::iter;
use std::sync::Arc;

pub const DEFAULT_CATALOG_NAME: &str = "datafusion";
const DEFAULT_SCHEMA_NAME: &str = "public";

#[derive(Debug)]
pub struct GrpcCatalogProvider {
    catalog_name: Option<String>,
    client: ConnectionClient,
    schemas: Mutex<HashMap<Option<String>, Arc<GrpcSchemaProvider>>>,
}

fn get_table_refs(client: &ConnectionClient) -> DataFusionResult<Vec<TableReference>> {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    let rt = builder.build().expect("failed to build tokio runtime");

    rt.block_on(async {
        Ok::<Vec<_>, DataFusionError>(
            client
                .clone()
                .inner()
                .find_entries(re_protos::cloud::v1alpha1::FindEntriesRequest {
                    filter: Some(EntryFilter {
                        entry_kind: Some(EntryKind::Table.into()),
                        ..Default::default()
                    }),
                })
                .await
                .map_err(|err| DataFusionError::External(Box::new(err)))?
                .into_inner()
                .entries
                .into_iter()
                .map(|entry| TableReference::from(entry.name()))
                .collect(),
        )
    })
}

impl GrpcCatalogProvider {
    pub fn new(name: Option<&str>, client: ConnectionClient) -> Self {
        let name = if let Some(inner_name) = name && inner_name == DEFAULT_CATALOG_NAME {
            None
        } else {
            name
        };
        let default_schema = Arc::new(GrpcSchemaProvider {
            catalog_name: name.map(ToOwned::to_owned),
            schema_name: None,
            client: client.clone(),
            in_memory_tables: Default::default(),
        });
        let schemas: HashMap<_, _> = iter::once((None, default_schema)).collect();

        Self {
            catalog_name: name.map(ToOwned::to_owned),
            client,
            schemas: Mutex::new(schemas),
        }
    }

    fn update_from_server(&self) -> DataFusionResult<()> {
        let table_names = get_table_refs(&self.client)?;

        let schema_names: HashSet<_> = table_names
            .into_iter()
            .filter(|table_ref| table_ref.catalog() == self.catalog_name.as_deref())
            .map(|table_ref| table_ref.schema().map(|s| s.to_owned()))
            .collect();

        let mut schemas = self.schemas.lock();

        schemas.retain(|k, _| schema_names.contains(k) || k.is_none());
        for schema_name in schema_names {
            let _ = schemas.entry(schema_name.clone()).or_insert(
                GrpcSchemaProvider {
                    catalog_name: self.catalog_name.clone(),
                    schema_name,
                    client: self.client.clone(),
                    in_memory_tables: Default::default(),
                }
                .into(),
            );
        }

        Ok(())
    }

    fn get_schema_names(&self) -> DataFusionResult<Vec<String>> {
        self.update_from_server()?;

        let schemas = self.schemas.lock();
        Ok(schemas
            .keys()
            .map(|k| k.as_deref().unwrap_or(DEFAULT_SCHEMA_NAME).to_owned())
            .collect())
    }
}

impl CatalogProvider for GrpcCatalogProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema_names(&self) -> Vec<String> {
        self.get_schema_names().unwrap_or_else(|err| {
            log::error!("Error attempting to get table references from server: {err}");
            vec![]
        })
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        if let Err(err) = self.update_from_server() {
            log::error!("Error updating table references from server: {err}");
            return None;
        }

        let schemas = self.schemas.lock();

        let schema_name = if name == DEFAULT_SCHEMA_NAME {
            None
        } else {
            Some(name.to_owned())
        };

        schemas
            .get(&schema_name)
            .map(|s| Arc::clone(s) as Arc<dyn SchemaProvider>)
    }
}

#[derive(Debug)]
struct GrpcSchemaProvider {
    catalog_name: Option<String>,
    schema_name: Option<String>,
    client: ConnectionClient,
    in_memory_tables: Mutex<HashMap<String, Arc<dyn TableProvider>>>,
}

impl GrpcSchemaProvider {}

#[async_trait]
impl SchemaProvider for GrpcSchemaProvider {
    fn owner_name(&self) -> Option<&str> {
        self.catalog_name.as_deref()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn table_names(&self) -> Vec<String> {
        let table_refs = get_table_refs(&self.client).unwrap_or_else(|err| {
            log::error!("Error getting table references: {err}");
            vec![]
        });

        let mut table_names = table_refs
            .into_iter()
            .filter(|table_ref| {
                table_ref.catalog() == self.catalog_name.as_deref()
                    && table_ref.schema() == self.schema_name.as_deref()
            })
            .map(|table_ref| table_ref.table().to_owned())
            .collect::<Vec<_>>();

        table_names.extend(self.in_memory_tables.lock().keys().cloned());

        table_names
    }

    async fn table(
        &self,
        table_name: &str,
    ) -> DataFusionResult<Option<Arc<dyn TableProvider>>, DataFusionError> {
        if let Some(table) = self.in_memory_tables.lock().get(table_name) {
            return Ok(Some(Arc::clone(table)));
        }

        let table_name = match (&self.catalog_name, &self.schema_name) {
            (Some(catalog_name), Some(schema_name)) => {
                format!("{catalog_name}.{schema_name}.{table_name}")
            }
            (None, Some(schema_name)) => format!("{schema_name}.{table_name}"),
            _ => table_name.to_owned(),
        };
        TableEntryTableProvider::new(self.client.clone(), table_name)
            .into_provider()
            .await
            .map(Some)
    }

    fn register_table(
        &self,
        name: String,
        table: Arc<dyn TableProvider>,
    ) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        let server_tables = get_table_refs(&self.client)?;
        if server_tables.into_iter().any(|table_ref| {
            table_ref.catalog() == self.catalog_name.as_deref()
                && table_ref.schema() == self.schema_name.as_deref()
                && table_ref.table() == name
        }) {
            return exec_err!("{name} already exists on the server catalog");
        }

        self.in_memory_tables.lock().insert(name, table);
        Ok(None)
    }

    fn deregister_table(&self, name: &str) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        Ok(self.in_memory_tables.lock().remove(name))
    }

    fn table_exist(&self, name: &str) -> bool {
        self.table_names().into_iter().any(|t| t == name)
    }
}
