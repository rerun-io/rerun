use std::any::Any;
use std::sync::Arc;

use ahash::{HashMap, HashSet};
use async_trait::async_trait;
use datafusion::catalog::{CatalogProvider, CatalogProviderList, SchemaProvider, TableProvider};
use datafusion::common::{DataFusionError, Result as DataFusionResult, TableReference, exec_err};
use datafusion::logical_expr::TableType;
use parking_lot::Mutex;
use re_log_types::EntryName;
use re_protos::cloud::v1alpha1::EntryKind;
use re_redap_client::ConnectionClient;
use re_uri::Origin;
use tokio::runtime::Handle as RuntimeHandle;

use crate::IntoDfError as _;
use crate::{TableEntryTableProvider, TableQueryCaller};

// These are to match the defaults in datafusion.
pub(crate) const DEFAULT_CATALOG_NAME: &str = "datafusion";
const DEFAULT_SCHEMA_NAME: &str = "public";

/// `DataFusion` catalog provider list for interacting with Rerun gRPC services.
///
/// Resolves catalog names lazily: no I/O on construction, no I/O in `catalog(name)`. SQL
/// planning never triggers a wildcard `FindEntries` on the catalog path. The wildcard is only
/// issued when `catalog_names()` is invoked (e.g. by `SHOW CATALOGS` or
/// `INFORMATION_SCHEMA.schemata`).
///
/// `catalog(name)` accepts any string and returns `Some(_)` (validation happens later, when
/// `table()` is resolved against the server). To prevent typos from leaking into
/// `catalog_names()` / `INFORMATION_SCHEMA.schemata`, lazily-minted providers are kept in a
/// separate `lazy_cache` that does **not** participate in name listings. Only catalogs the
/// server actually knows about and catalogs explicitly added via
/// [`CatalogProviderList::register_catalog`] surface.
#[derive(Debug)]
pub struct RedapCatalogProviderList {
    client: ConnectionClient,
    runtime: RuntimeHandle,
    analytics_origin: Option<Origin>,

    /// Catalogs explicitly added via [`CatalogProviderList::register_catalog`]. Listed in
    /// `catalog_names()`.
    registered: Mutex<HashMap<String, Arc<dyn CatalogProvider>>>,

    /// Catalogs minted on-demand by `catalog(name)` for any name the planner asks for. NOT
    /// listed in `catalog_names()` — including these would surface typos as if they were real
    /// catalogs. Bounded in practice by the set of distinct catalog names the planner has
    /// touched in this session.
    lazy_cache: Mutex<HashMap<String, Arc<dyn CatalogProvider>>>,
}

impl RedapCatalogProviderList {
    pub fn new(
        client: ConnectionClient,
        runtime: RuntimeHandle,
        analytics_origin: Option<Origin>,
    ) -> Self {
        Self {
            client,
            runtime,
            analytics_origin,
            registered: Mutex::new(HashMap::default()),
            lazy_cache: Mutex::new(HashMap::default()),
        }
    }
}

impl CatalogProviderList for RedapCatalogProviderList {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn register_catalog(
        &self,
        name: String,
        catalog: Arc<dyn CatalogProvider>,
    ) -> Option<Arc<dyn CatalogProvider>> {
        // The explicit registration shadows any lazy-minted provider of the same name.
        self.lazy_cache.lock().remove(&name);
        self.registered.lock().insert(name, catalog)
    }

    fn catalog_names(&self) -> Vec<String> {
        // Wildcard `FindEntries(kind=Table)` on demand. Only `SHOW CATALOGS` /
        // `INFORMATION_SCHEMA.schemata` invoke this; SELECT planning resolves through
        // `catalog(name)` and never enumerates.
        let mut names: HashSet<String> = match get_table_refs(&self.client, &self.runtime) {
            Ok(refs) => refs
                .into_iter()
                .filter_map(|t| t.catalog().map(ToOwned::to_owned))
                .collect(),
            Err(err) => {
                re_log::error!("Error attempting to get catalog names from server: {err}");
                HashSet::default()
            }
        };

        // The default catalog is always present; explicitly registered catalogs (e.g. in-memory
        // ones inserted via `ctx.register_catalog`) must also surface. Lazily-minted entries
        // are deliberately excluded.
        names.insert(DEFAULT_CATALOG_NAME.to_owned());
        names.extend(self.registered.lock().keys().cloned());
        names.into_iter().collect()
    }

    fn catalog(&self, name: &str) -> Option<Arc<dyn CatalogProvider>> {
        // Explicit registrations take precedence.
        if let Some(provider) = self.registered.lock().get(name) {
            return Some(Arc::clone(provider));
        }

        let mut lazy = self.lazy_cache.lock();
        let provider = lazy.entry(name.to_owned()).or_insert_with(|| {
            Arc::new(RedapCatalogProvider::new(
                Some(name),
                self.client.clone(),
                self.runtime.clone(),
                self.analytics_origin.clone(),
            )) as Arc<dyn CatalogProvider>
        });
        Some(Arc::clone(provider))
    }
}

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
pub(crate) struct RedapCatalogProvider {
    catalog_name: Option<String>,
    client: ConnectionClient,

    /// Lazy cache of schema providers keyed by schema name (`None` for the default schema).
    /// Populated on demand by `schema(name)`; never invalidated. If a schema is deleted
    /// server-side its entry remains here until the parent `RedapCatalogProvider` is dropped.
    /// Not a correctness issue: every `SchemaProvider::table*` call rechecks the server via a
    /// name-filtered `FindEntries`, so a stale entry can never fabricate data. Just a small
    /// memory cost bounded by the number of distinct schema names the planner has touched.
    schemas: Mutex<HashMap<Option<String>, Arc<RedapSchemaProvider>>>,
    runtime: RuntimeHandle,

    /// When set, table-scan analytics are emitted to this cloud origin for any
    /// table resolved through this catalog. `None` ⇒ no analytics.
    analytics_origin: Option<Origin>,
}

#[tracing::instrument(skip_all)]
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
                .map_err(|err| err.into_df_error())?
                .into_iter()
                .map(|name| TableReference::from(name.to_string()))
                .collect(),
        )
    })
}

impl RedapCatalogProvider {
    pub(crate) fn new(
        name: Option<&str>,
        client: ConnectionClient,
        runtime: RuntimeHandle,
        analytics_origin: Option<Origin>,
    ) -> Self {
        let catalog_name = name
            .filter(|n| *n != DEFAULT_CATALOG_NAME)
            .map(ToOwned::to_owned);

        Self {
            catalog_name,
            client,
            schemas: Mutex::new(HashMap::default()),
            runtime,
            analytics_origin,
        }
    }
}

impl CatalogProvider for RedapCatalogProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema_names(&self) -> Vec<String> {
        // Enumerates server entries (a wildcard `FindEntries`) and projects them to distinct
        // schema names. Only used by `SHOW SCHEMAS` / `INFORMATION_SCHEMA.schemata`; the SELECT
        // planning path resolves through `schema(name)` and never reaches this method.
        let table_refs = match get_table_refs(&self.client, &self.runtime) {
            Ok(refs) => refs,
            Err(err) => {
                re_log::error!("Error attempting to get table references from server: {err}");
                return vec![];
            }
        };

        let mut schema_keys: HashSet<&str> = table_refs
            .iter()
            .filter(|table_ref| table_ref.catalog() == self.catalog_name.as_deref())
            .filter_map(|table_ref| table_ref.schema())
            .collect();

        // The default schema is always present, even if no tables live in it yet.
        schema_keys.insert(DEFAULT_SCHEMA_NAME);

        schema_keys.into_iter().map(str::to_owned).collect()
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        let schema_name: Option<String> = if name == DEFAULT_SCHEMA_NAME {
            None
        } else {
            Some(name.to_owned())
        };

        let mut schemas = self.schemas.lock();
        let provider = schemas.entry(schema_name.clone()).or_insert_with(|| {
            Arc::new(RedapSchemaProvider {
                catalog_name: self.catalog_name.clone(),
                schema_name,
                client: self.client.clone(),
                runtime: self.runtime.clone(),
                in_memory_tables: Default::default(),
                analytics_origin: self.analytics_origin.clone(),
            })
        });
        Some(Arc::clone(provider) as Arc<dyn SchemaProvider>)
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

    /// Inherited from `RedapCatalogProvider`. `Some` ⇒ enable analytics for
    /// providers constructed by `table()`.
    analytics_origin: Option<Origin>,
}

/// Reconstruct the dotted entry name as stored on the server from a provider's catalog/schema
/// and the leaf table name.
fn full_table_name(catalog: Option<&str>, schema: Option<&str>, table_name: &str) -> String {
    match (catalog, schema) {
        (Some(catalog), Some(schema)) => format!("{catalog}.{schema}.{table_name}"),
        (None, Some(schema)) => format!("{schema}.{table_name}"),
        _ => table_name.to_owned(),
    }
}

impl RedapSchemaProvider {
    fn full_table_name(&self, table_name: &str) -> String {
        full_table_name(
            self.catalog_name.as_deref(),
            self.schema_name.as_deref(),
            table_name,
        )
    }

    /// Ask the server whether a `Table` entry exists by name. Uses a name+kind-filtered
    /// `FindEntries` (point lookup), never a wildcard scan. The server returns gRPC `NotFound`
    /// when no entry matches; we map that to `Ok(false)` rather than propagating the error.
    async fn lookup_table_on_server_async(&self, table_name: &str) -> DataFusionResult<bool> {
        let entry_name = EntryName::new(self.full_table_name(table_name))
            .map_err(|err| DataFusionError::Plan(format!("invalid entry name: {err}")))?;

        let mut client = self.client.clone();
        match client
            .get_entry_id(&entry_name, Some(EntryKind::Table))
            .await
        {
            Ok(opt) => Ok(opt.is_some()),
            Err(err) if err.kind == re_redap_client::ApiErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into_df_error()),
        }
    }

    /// Sync wrapper around [`Self::lookup_table_on_server_async`] for the sync trait methods
    /// (`table_exist`, `register_table`). Must NOT be called from an async context driven by
    /// `self.runtime` — `Handle::block_on` panics in that case. Async trait methods such as
    /// `table_type` should call `lookup_table_on_server_async` directly.
    fn lookup_table_on_server(&self, table_name: &str) -> DataFusionResult<bool> {
        self.runtime
            .block_on(self.lookup_table_on_server_async(table_name))
    }
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
            re_log::error!("Error getting table references: {err}");
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

        let mut provider = TableEntryTableProvider::new(
            self.client.clone(),
            self.full_table_name(table_name),
            Some(self.runtime.clone()),
        )
        .with_caller(TableQueryCaller::CatalogResolver);
        if let Some(origin) = self.analytics_origin.clone() {
            provider = provider.with_analytics(origin);
        }
        provider.into_provider().await.map(Some)
    }

    fn register_table(
        &self,
        name: String,
        table: Arc<dyn TableProvider>,
    ) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        if self.lookup_table_on_server(&name)? {
            return exec_err!("{name} already exists on the server catalog");
        }

        self.in_memory_tables.lock().insert(name, table);
        Ok(None)
    }

    fn deregister_table(&self, name: &str) -> DataFusionResult<Option<Arc<dyn TableProvider>>> {
        Ok(self.in_memory_tables.lock().remove(name))
    }

    fn table_exist(&self, name: &str) -> bool {
        if self.in_memory_tables.lock().contains_key(name) {
            return true;
        }

        self.lookup_table_on_server(name).unwrap_or_else(|err| {
            re_log::error!("Error checking table existence for {name}: {err}");
            false
        })
    }

    /// Server-resolved tables are always `TableType::Base`. We override this method (the trait's
    /// default impl calls `self.table(name).await`, which builds a fresh `TableEntryTableProvider`
    /// and triggers a `GetTableSchema` round-trip per call) so that
    /// `INFORMATION_SCHEMA.tables` and other introspection paths only pay the cost of a
    /// name-filtered `FindEntries`.
    ///
    /// Routes through the `_async` lookup so that this future can be polled on the same runtime
    /// stored in `self.runtime` (the production case via `FFI_CatalogProviderList`); calling the
    /// sync `lookup_table_on_server` here would panic inside `Handle::block_on`.
    async fn table_type(&self, name: &str) -> DataFusionResult<Option<TableType>> {
        if let Some(table) = self.in_memory_tables.lock().get(name) {
            return Ok(Some(table.table_type()));
        }

        if self.lookup_table_on_server_async(name).await? {
            Ok(Some(TableType::Base))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::full_table_name;

    #[test]
    fn full_table_name_combines_catalog_schema_table() {
        assert_eq!(
            full_table_name(Some("cat"), Some("schema"), "tbl"),
            "cat.schema.tbl"
        );
        assert_eq!(full_table_name(None, Some("schema"), "tbl"), "schema.tbl");
        assert_eq!(full_table_name(None, None, "tbl"), "tbl");
        // Catalog without a schema falls through to the bare-table case (matches the legacy
        // `RedapSchemaProvider::table()` reconstruction).
        assert_eq!(full_table_name(Some("cat"), None, "tbl"), "tbl");
    }
}
