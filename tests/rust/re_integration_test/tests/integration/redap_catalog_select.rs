//! Behavior tests for `RedapCatalogProviderList`: a real `TestServer`, real `ConnectionClient`,
//! real `SessionContext`. Verifies that SELECT (qualified and unqualified), `table_exist`, and
//! `register_table` collision detection still work end-to-end with a single lazy provider list.

use std::sync::Arc;

use arrow::array::{Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::catalog::TableProvider;
use datafusion::datasource::MemTable;
use datafusion::logical_expr::TableType;
use datafusion::prelude::SessionContext;
use re_datafusion::RedapCatalogProviderList;
use re_integration_test::TestServer;
use re_protos::cloud::v1alpha1::ext::TableInsertMode;

const FLAT_TABLE: &str = "flat_table";
const QUALIFIED_TABLE: &str = "cat.schema.qualified_table";

#[tokio::test(flavor = "multi_thread")]
async fn select_via_redap_catalog_provider_list() {
    let server = TestServer::spawn().await;
    let client = server.client().await.expect("connect");

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ],
        Default::default(),
    ));

    create_and_populate(&client, FLAT_TABLE, &schema).await;
    create_and_populate(&client, QUALIFIED_TABLE, &schema).await;

    let runtime = tokio::runtime::Handle::current();
    let ctx = Arc::new(SessionContext::new());
    ctx.register_catalog_list(Arc::new(RedapCatalogProviderList::new(
        client, runtime, None,
    )));

    // SELECT planning resolves through the async `SchemaProvider::table` path, so it does not
    // hit the sync `block_on` in `lookup_table_on_server`. Run directly on the async test.
    let count = run_count(&ctx, &format!("SELECT COUNT(*) FROM {FLAT_TABLE}")).await;
    assert_eq!(count, 3);

    let count = run_count(&ctx, &format!("SELECT COUNT(*) FROM {QUALIFIED_TABLE}")).await;
    assert_eq!(count, 3);

    // `table_exist` and `register_table` are sync trait methods that call `block_on` internally.
    // From inside a tokio task that would panic, so escape the async context via
    // `spawn_blocking`. (Production callers — the Python SDK and DataFusion's DDL paths — invoke
    // these from non-task threads.)
    let ctx_blocking = Arc::clone(&ctx);
    let mem_table_schema = Arc::clone(&schema);
    tokio::task::spawn_blocking(move || {
        let cat = ctx_blocking
            .catalog("cat")
            .expect("catalog `cat` registered");
        let schema_provider = cat
            .schema("schema")
            .expect("schema `schema` resolved lazily");

        assert!(schema_provider.table_exist("qualified_table"));
        assert!(!schema_provider.table_exist("definitely_not_a_table"));

        let mem_table: Arc<dyn TableProvider> =
            Arc::new(MemTable::try_new(mem_table_schema, vec![vec![]]).expect("mem table"));
        assert!(
            schema_provider
                .register_table("qualified_table".to_owned(), Arc::clone(&mem_table))
                .is_err(),
            "register_table must reject names that already exist server-side"
        );
        let result =
            schema_provider.register_table("brand_new_in_memory_table".to_owned(), mem_table);
        assert!(
            result.is_ok(),
            "register_table must accept fresh names; got {result:?}"
        );
    })
    .await
    .expect("spawn_blocking task panicked");
}

async fn create_and_populate(
    client: &re_redap_client::ConnectionClient,
    name: &str,
    schema: &Arc<Schema>,
) {
    let mut client = client.clone();
    let entry_name =
        re_log_types::EntryName::new(name).expect("test name must be a valid EntryName");

    let table = client
        .create_table_entry(entry_name, None, schema.clone())
        .await
        .expect("create_table_entry");

    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["alpha", "beta", "gamma"])),
        ],
        &Default::default(),
    )
    .expect("record batch");

    client
        .write_table(
            futures::stream::once(async { batch }),
            table.details.id,
            TableInsertMode::Append,
        )
        .await
        .expect("write_table");
}

async fn run_count(ctx: &SessionContext, sql: &str) -> i64 {
    let df = ctx.sql(sql).await.expect("sql plan");
    let batches = df.collect().await.expect("collect");
    let batch = batches
        .into_iter()
        .find(|b| b.num_rows() > 0)
        .expect("at least one batch with rows");
    batch
        .column(0)
        .as_any()
        .downcast_ref::<Int64Array>()
        .expect("count column is i64")
        .value(0)
}

// `table_type` is an `async` trait method that historically routed through
// `lookup_table_on_server`, which calls `self.runtime.block_on(...)`. When the
// future was polled on a worker of the same runtime stored in `self.runtime` —
// the production case via `FFI_CatalogProviderList` — `Handle::block_on`
// panicked, crashing any path that awaited `table_type` (notably
// `INFORMATION_SCHEMA.tables`).
//
// The fix routes `table_type` through `lookup_table_on_server_async` so the
// future can be polled on the same runtime without re-entering `block_on`.
// This test pins that contract: install `RedapCatalogProviderList` on the test
// runtime via `Handle::current()`, then await `table_type` directly. It must
// resolve to `Some(TableType::Base)` for an existing table and `None` for a
// missing one — never panic.
#[tokio::test(flavor = "multi_thread")]
async fn table_type_resolves_when_polled_on_provider_runtime() {
    let server = TestServer::spawn().await;
    let client = server.client().await.expect("connect");

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ],
        Default::default(),
    ));

    create_and_populate(&client, QUALIFIED_TABLE, &schema).await;

    let runtime = tokio::runtime::Handle::current();
    let ctx = Arc::new(SessionContext::new());
    ctx.register_catalog_list(Arc::new(RedapCatalogProviderList::new(
        client, runtime, None,
    )));

    let cat = ctx.catalog("cat").expect("catalog `cat` registered");
    let schema_provider = cat
        .schema("schema")
        .expect("schema `schema` resolved lazily");

    let existing = schema_provider
        .table_type("qualified_table")
        .await
        .expect("table_type must not error for an existing entry");
    assert_eq!(existing, Some(TableType::Base));

    let missing = schema_provider
        .table_type("definitely_not_a_table")
        .await
        .expect("table_type must map server NotFound to Ok(None)");
    assert_eq!(missing, None);
}
