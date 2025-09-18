use crate::{DataframeQueryTableProvider, TableEntryTableProvider};
use ahash::{HashMap, HashSet};
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, SchemaProvider, TableProvider};
use datafusion::common::{
    DataFusionError, Result as DataFusionResult, TableReference, exec_datafusion_err,
};
use parking_lot::Mutex;
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_redap_client::ConnectionClient;
use std::any::Any;
use std::sync::Arc;

const DEFAULT_CATALOG_NAME: &str = "rerun";
const DEFAULT_SCHEMA_NAME: &str = "default";

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
    fn update_from_server(&self) -> DataFusionResult<()> {
        let table_names = get_table_refs(&self.client)?;

        let schema_names: HashSet<_> = table_names
            .into_iter()
            .filter(|table_ref| table_ref.catalog() == self.catalog_name.as_deref())
            .map(|table_ref| table_ref.schema().map(|s| s.to_owned()))
            .collect();

        let mut schemas = self.schemas.lock();

        schemas.retain(|k, _| schema_names.contains(k));
        for schema_name in schema_names {
            let _ = schemas.entry(schema_name.clone()).or_insert(
                GrpcSchemaProvider {
                    catalog_name: self.catalog_name.clone(),
                    schema_name,
                    client: self.client.clone(),
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

#[derive(Debug, Clone)]
struct GrpcSchemaProvider {
    catalog_name: Option<String>,
    schema_name: Option<String>,
    client: ConnectionClient,
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

        table_refs
            .into_iter()
            .filter(|table_ref| {
                table_ref.catalog() == self.catalog_name.as_deref()
                    && table_ref.schema() == self.schema_name.as_deref()
            })
            .map(|table_ref| table_ref.table().to_owned())
            .collect()
    }

    async fn table(
        &self,
        table_name: &str,
    ) -> DataFusionResult<Option<Arc<dyn TableProvider>>, DataFusionError> {
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
        todo!()
    }

    fn deregister_table(&self, name: &str) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        todo!()
    }

    fn table_exist(&self, name: &str) -> bool {
        todo!()
    }
}
