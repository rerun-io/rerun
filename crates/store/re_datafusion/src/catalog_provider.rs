use std::any::Any;
use std::iter;
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, TableProvider};
use datafusion::common::{DataFusionError, Result as DataFusionResult, TableReference, exec_err};
use parking_lot::Mutex;
use re_redap_client::ConnectionClient;
use tokio::runtime::Handle as RuntimeHandle;

use crate::TableEntryTableProvider;

// These are to match the defaults in datafusion.
pub const DEFAULT_CATALOG_NAME: &str = "datafusion";
const DEFAULT_SCHEMA_NAME: &str = "public";

/// `DataFusion` catalog provider for interacting with Rerun gRPC services.
///
/// Tables are stored on the server in a flat namespace with a string
/// representation of the catalog, schema, and table delimited by a
/// period. This matches typical SQL style naming conventions. It the
/// catalog or schema is not specified, it will be assumed to use
/// the defaults. For example a table stored with table named
/// `my_table` will be stored within the `datafusion` catalog and
/// `public` schema. If a table is specified with more than three
/// levels, it will also be stored in the default catalog and schema.
/// This matches how `DataFusion` will store such table names.
#[derive(Debug)]
pub struct RedapCatalogProvider {
    catalog_name: Option<String>,
    client: ConnectionClient,
    schemas: Mutex<HashMap<Option<String>, Arc<RedapSchemaProvider>>>,
    runtime: RuntimeHandle,
}

fn get_table_refs(
    client: &ConnectionClient,
    runtime: &RuntimeHandle,
) -> DataFusionResult<Vec<TableReference>> {
    runtime.block_on(async {
        Ok::<Vec<_>, DataFusionError>(
            client
                .clone()
                .get_table_names()
                .await
                .map_err(|err| DataFusionError::External(Box::new(err)))?
                .into_iter()
                .map(TableReference::from)
                .collect(),
        )
    })
}

pub fn get_all_catalog_names(
    client: &ConnectionClient,
    runtime: &RuntimeHandle,
) -> DataFusionResult<Vec<String>> {
    let catalog_names = get_table_refs(client, runtime)?
        .into_iter()
        .filter_map(|reference| reference.catalog().map(|c| c.to_owned()))
        .collect::<HashSet<String>>();

    Ok(catalog_names.into_iter().collect())
}

impl RedapCatalogProvider {
    pub fn new(name: Option<&str>, client: ConnectionClient, runtime: RuntimeHandle) -> Self {
        let name = if let Some(inner_name) = name
            && inner_name == DEFAULT_CATALOG_NAME
        {
            None
        } else {
            name
        };
        let default_schema = Arc::new(RedapSchemaProvider {
            catalog_name: name.map(ToOwned::to_owned),
            schema_name: None,
            client: client.clone(),
            runtime: runtime.clone(),
            in_memory_tables: Default::default(),
        });
        let schemas: HashMap<_, _> = iter::once((None, default_schema)).collect();

        Self {
            catalog_name: name.map(ToOwned::to_owned),
            client,
            schemas: Mutex::new(schemas),
            runtime,
        }
    }

    fn update_from_server(&self) -> DataFusionResult<()> {
        let table_names = get_table_refs(&self.client, &self.runtime)?;

        let schema_names: HashSet<_> = table_names
            .into_iter()
            .filter(|table_ref| table_ref.catalog() == self.catalog_name.as_deref())
            .map(|table_ref| table_ref.schema().map(|s| s.to_owned()))
            .collect();

        let mut schemas = self.schemas.lock();

        schemas.retain(|k, _| schema_names.contains(k) || k.is_none());
        for schema_name in schema_names {
            let _ = schemas.entry(schema_name.clone()).or_insert(
                RedapSchemaProvider {
                    catalog_name: self.catalog_name.clone(),
                    schema_name,
                    client: self.client.clone(),
                    runtime: self.runtime.clone(),
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

impl CatalogProvider for RedapCatalogProvider {
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

/// `DataFusion` schema provider for interacting with Rerun gRPC services.
///
/// For a detailed description of how tables are named on the server
/// vs represented in the catalog and schema providers, see
/// [`RedapCatalogProvider`].
///
/// When the user calls `register_table` on this provider, it will
/// register the table *only for the current session*. To persist
/// tables, instead they must be registered via the [`ConnectionClient`]
/// `register_table`. It is expected for this behavior to change in
/// the future.
#[derive(Debug)]
struct RedapSchemaProvider {
    catalog_name: Option<String>,
    schema_name: Option<String>,
    client: ConnectionClient,
    runtime: RuntimeHandle,
    in_memory_tables: Mutex<HashMap<String, Arc<dyn TableProvider>>>,
}

#[async_trait]
impl SchemaProvider for RedapSchemaProvider {
    fn owner_name(&self) -> Option<&str> {
        self.catalog_name.as_deref()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn table_names(&self) -> Vec<String> {
        let table_refs = get_table_refs(&self.client, &self.runtime).unwrap_or_else(|err| {
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
        TableEntryTableProvider::new(self.client.clone(), table_name, Some(self.runtime.clone()))
            .into_provider()
            .await
            .map(Some)
    }

    fn register_table(
        &self,
        name: String,
        table: Arc<dyn TableProvider>,
    ) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        let server_tables = get_table_refs(&self.client, &self.runtime)?;
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
