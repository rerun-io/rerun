//! The catalog tools against an in-process catalog server.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use strum::VariantArray as _;

use re_catalog_tools::{CatalogTool, CatalogTools};
use re_log_types::{EntryName, TimeType};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{Connection, ConnectionRegistry, ConnectionRegistryHandle};
use re_redap_tests::{TempPath, create_simple_recording};

const DATASET: &str = "robots";

/// Tools connected to a catalog holding the dataset [`DATASET`] with two segments.
///
/// The recordings must outlive the server, which reads them on demand.
async fn connected_tools() -> (CatalogTools, Vec<TempPath>, tempfile::TempDir) {
    let (tools, _registry, recordings, storage) = catalog_server().await;
    (tools, recordings, storage)
}

/// Like [`connected_tools`], also returning the registry that reaches the catalog.
async fn catalog_server() -> (
    CatalogTools,
    ConnectionRegistryHandle,
    Vec<TempPath>,
    tempfile::TempDir,
) {
    let recordings = vec![
        create_simple_recording(
            1,
            "segment_a",
            &["/points", "/camera"],
            0,
            TimeType::Sequence,
        )
        .expect("recording"),
        create_simple_recording(2, "segment_b", &["/points"], 0, TimeType::Sequence)
            .expect("recording"),
    ];
    let handler = re_server::RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!())
        .with_rrds_as_dataset(
            EntryName::new(DATASET).expect("valid name"),
            recordings.iter().map(|path| path.to_path_buf()).collect(),
            IfDuplicateBehavior::Error,
            re_server::OnError::Abort,
        )
        .await
        .expect("dataset")
        .build()
        .expect("handler");

    let storage = tempfile::tempdir().expect("temp dir");
    let origin = re_uri::Origin::http_local_host(1);
    let registry = ConnectionRegistry::new_without_stored_credentials().with_internal(
        Connection::from_service(origin.clone(), Arc::new(handler), re_server::capabilities()),
        storage.path().to_path_buf(),
    );
    let tools = CatalogTools::new(registry.clone(), runtime(), Some(origin));
    (tools, registry, recordings, storage)
}

fn runtime() -> re_async::AsyncRuntimeHandle {
    re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime")
}

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => panic!("arguments must be an object: {value}"),
    }
}

async fn segment_page(tools: &CatalogTools, arguments: Value) -> Value {
    tools
        .run(CatalogTool::ListSegments, args(arguments))
        .await
        .expect("segments")
}

#[tokio::test(flavor = "multi_thread")]
async fn lists_and_describes_a_dataset() {
    let (tools, _recordings, _storage) = connected_tools().await;

    let entries = tools
        .run(CatalogTool::ListEntries, Map::new())
        .await
        .expect("entries");
    let dataset = entries["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["name"] == DATASET)
        .expect("the dataset is listed");
    assert_eq!(dataset["kind"], "dataset");
    let kinds: Vec<&Value> = entries["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| &entry["kind"])
        .collect();
    assert!(kinds.contains(&&json!("table")), "{entries}");
    let second = tools
        .run(
            CatalogTool::ListEntries,
            args(json!({ "offset": 1, "limit": 1 })),
        )
        .await
        .expect("entries");
    assert_eq!(second["num_entries"], entries["num_entries"]);
    assert_eq!(second["entries"], json!([entries["entries"][1]]));
    let tables = tools
        .run(CatalogTool::ListEntries, args(json!({ "kind": "table" })))
        .await
        .expect("entries");
    assert!(
        tables["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .all(|entry| entry["kind"] == "table"),
        "{tables}"
    );

    let info = tools
        .run(
            CatalogTool::GetDatasetInfo,
            args(json!({ "dataset": DATASET })),
        )
        .await
        .expect("info");
    assert_eq!(info["num_segments"], 2);

    // By id, as well as by name.
    let id = dataset["id"].as_str().expect("id").to_owned();
    let schema = tools
        .run(
            CatalogTool::GetDatasetSchema,
            args(json!({ "dataset": id })),
        )
        .await
        .expect("schema");
    let entities = schema["entities"].as_object().expect("entities");
    assert!(entities.contains_key("/points"), "{schema}");
    assert!(entities.contains_key("/camera"), "{schema}");
    let past_the_first = tools
        .run(
            CatalogTool::GetDatasetSchema,
            args(json!({ "dataset": id, "offset": 1 })),
        )
        .await
        .expect("schema");
    assert_eq!(past_the_first["num_entities"], schema["num_entities"]);
    assert_eq!(
        past_the_first["entities"].as_object().map(|e| e.len()),
        Some(entities.len() - 1)
    );

    let points = tools
        .run(
            CatalogTool::GetDatasetSchema,
            args(json!({ "dataset": DATASET, "entity_path": "/points" })),
        )
        .await
        .expect("schema");
    assert!(
        !points["components"]
            .as_array()
            .expect("components")
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pages_through_segments() {
    let (tools, _recordings, _storage) = connected_tools().await;

    assert_eq!(
        segment_page(&tools, json!({ "dataset": DATASET })).await,
        json!({ "num_segments": 2, "offset": 0, "segment_ids": ["segment_a", "segment_b"] })
    );
    assert_eq!(
        segment_page(
            &tools,
            json!({ "dataset": DATASET, "offset": 1, "limit": 5 })
        )
        .await,
        json!({ "num_segments": 2, "offset": 1, "segment_ids": ["segment_b"] })
    );
    assert_eq!(
        segment_page(&tools, json!({ "dataset": DATASET, "limit": 1 })).await,
        json!({ "num_segments": 2, "offset": 0, "segment_ids": ["segment_a"] })
    );
    assert_eq!(
        segment_page(&tools, json!({ "dataset": DATASET, "offset": 5 })).await,
        json!({ "num_segments": 2, "offset": 5, "segment_ids": [] })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_segment_properties() {
    use arrow::array::{ListBuilder, RecordBatch, StringBuilder};

    let (tools, registry, _recordings, _storage) = catalog_server().await;

    let empty = tools
        .run(
            CatalogTool::GetSegmentProperties,
            args(json!({ "dataset": DATASET, "segment_id": "segment_a" })),
        )
        .await
        .expect("properties");
    assert_eq!(
        empty,
        json!({ "segment_id": "segment_a", "properties": {} })
    );

    let origin = tools.origin().expect("connected");
    let catalog =
        re_catalog_client::CatalogClient::new(registry.connection_handle(origin), runtime());
    let entries = tools
        .run(CatalogTool::ListEntries, Map::new())
        .await
        .expect("entries");
    let dataset_id: re_log_types::EntryId = entries["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["name"] == DATASET)
        .expect("the dataset is listed")["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("valid id");
    let mut names = ListBuilder::new(StringBuilder::new());
    names.values().append_value("wall-e");
    names.append(true);
    let properties = RecordBatch::try_from_iter([(
        "property:robot:name",
        std::sync::Arc::new(names.finish()) as _,
    )])
    .expect("batch");
    catalog
        .set_segment_properties(dataset_id, "segment_a".into(), &properties, None)
        .await
        .expect("set properties");

    let properties = tools
        .run(
            CatalogTool::GetSegmentProperties,
            args(json!({ "dataset": DATASET, "segment_id": "segment_a" })),
        )
        .await
        .expect("properties");
    assert_eq!(
        properties,
        json!({
            "segment_id": "segment_a",
            "properties": { "property:robot:name": ["wall-e"] },
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_switches_catalogs_only_once_reached() {
    let (tools, _recordings, _storage) = connected_tools().await;
    let origin = tools.origin().expect("connected");

    let err = tools
        .run(
            CatalogTool::Connect,
            args(json!({ "url": "ftp://example.com" })),
        )
        .await
        .expect_err("invalid url");
    assert!(err.to_string().contains("ftp://example.com"), "{err}");

    // Nothing listens on port 2.
    let err = tools
        .run(
            CatalogTool::Connect,
            args(json!({ "url": "rerun+http://127.0.0.1:2" })),
        )
        .await
        .expect_err("unreachable");
    assert!(err.to_string().contains("connect to the catalog"), "{err}");
    assert_eq!(tools.origin(), Some(origin.clone()));

    let connected = tools
        .run(
            CatalogTool::Connect,
            args(json!({ "url": origin.to_string() })),
        )
        .await
        .expect("connected");
    assert_eq!(connected["connected"], origin.to_string());

    // The agent's choice sticks.
    tools.connect_default(re_uri::Origin::http_local_host(2));
    assert_eq!(tools.origin(), Some(origin));
}

/// A catalog server on a local TCP port, holding one dataset named `dataset`, answering every
/// request after `latency`.
async fn tcp_catalog(
    dataset: &str,
    latency: std::time::Duration,
) -> (re_uri::Origin, re_server::ServerHandle, TempPath) {
    use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudServiceServer;

    let recording = create_simple_recording(3, "segment_c", &["/points"], 0, TimeType::Sequence)
        .expect("recording");
    let handler = re_server::RerunCloudHandlerBuilder::new(re_build_info::cargo_build_info!())
        .with_rrds_as_dataset(
            EntryName::new(dataset).expect("valid name"),
            vec![recording.to_path_buf()],
            IfDuplicateBehavior::Error,
            re_server::OnError::Abort,
        )
        .await
        .expect("dataset")
        .build()
        .expect("handler");
    let server = re_server::ServerBuilder::bind((std::net::Ipv4Addr::LOCALHOST, 0).into())
        .expect("bind")
        .with_service(RerunCloudServiceServer::new(handler))
        .with_artificial_latency(latency)
        .build()
        .start(&runtime())
        .await
        .expect("server");
    let origin = format!("rerun+http://{}", server.connect_addr())
        .parse()
        .expect("origin");
    (origin, server, recording)
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_replaces_the_catalog() {
    let (tools, _recordings, _storage) = connected_tools().await;
    let (other_origin, _server, _recording) = tcp_catalog("other", std::time::Duration::ZERO).await;

    tools
        .run(
            CatalogTool::Connect,
            args(json!({ "url": other_origin.to_string() })),
        )
        .await
        .expect("connected");
    assert_eq!(tools.origin(), Some(other_origin.clone()));

    let entries = tools
        .run(CatalogTool::ListEntries, args(json!({ "kind": "dataset" })))
        .await
        .expect("entries");
    assert_eq!(entries["catalog"], other_origin.to_string());
    let names: Vec<&Value> = entries["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| &entry["name"])
        .collect();
    assert_eq!(names, [&json!("other")]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_connect_issued_last_wins() {
    let (tools, _recordings, _storage) = connected_tools().await;
    let fast = tools.origin().expect("connected");
    let (slow, _server, _recording) =
        tcp_catalog("slow", std::time::Duration::from_millis(500)).await;

    let first = {
        let tools = tools.clone();
        let url = slow.to_string();
        tokio::spawn(async move {
            tools
                .run(CatalogTool::Connect, args(json!({ "url": url })))
                .await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    tools
        .run(
            CatalogTool::Connect,
            args(json!({ "url": fast.to_string() })),
        )
        .await
        .expect("fast connect");

    let err = first.await.expect("join").expect_err("superseded");
    assert!(err.to_string().contains("later `catalog_connect`"), "{err}");
    assert_eq!(tools.origin(), Some(fast));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_catalog_changes_until_the_agent_connects() {
    let tools = CatalogTools::new(
        ConnectionRegistry::new_without_stored_credentials(),
        runtime(),
        None,
    );
    assert_eq!(tools.origin(), None);
    let first = re_uri::Origin::http_local_host(1);
    let second = re_uri::Origin::http_local_host(2);
    tools.connect_default(first.clone());
    assert_eq!(tools.origin(), Some(first));
    tools.connect_default(second.clone());
    assert_eq!(tools.origin(), Some(second));
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_say_what_to_do_next() {
    let (tools, _recordings, _storage) = connected_tools().await;

    let err = tools
        .run(
            CatalogTool::GetDatasetInfo,
            args(json!({ "dataset": "nope" })),
        )
        .await
        .expect_err("no such dataset");
    assert!(err.to_string().contains("catalog_list_entries"), "{err}");

    let err = tools
        .run(
            CatalogTool::GetDatasetSchema,
            args(json!({ "dataset": DATASET, "entity_path": "/nope" })),
        )
        .await
        .expect_err("no such entity");
    assert!(err.to_string().contains("without `entity_path`"), "{err}");

    let err = tools
        .run(
            CatalogTool::GetSegmentProperties,
            args(json!({ "dataset": DATASET, "segment_id": "nope" })),
        )
        .await
        .expect_err("no such segment");
    assert!(err.to_string().contains("catalog_list_segments"), "{err}");

    let err = tools
        .run(CatalogTool::ListEntries, args(json!({ "typo": 1 })))
        .await
        .expect_err("unknown argument");
    assert!(err.to_string().contains("typo"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dataset_is_found_only_by_its_id_or_name() {
    let (tools, registry, _recordings, _storage) = catalog_server().await;

    let unknown_id = re_log_types::EntryId::new().to_string();
    let tables = tools
        .run(CatalogTool::ListEntries, args(json!({ "kind": "table" })))
        .await
        .expect("tables");
    let table_id = tables["entries"][0]["id"]
        .as_str()
        .expect("a table")
        .to_owned();
    for not_a_dataset in [unknown_id, table_id] {
        let err = tools
            .run(
                CatalogTool::GetDatasetInfo,
                args(json!({ "dataset": not_a_dataset })),
            )
            .await
            .expect_err("not a dataset");
        assert!(err.to_string().contains("catalog_list_entries"), "{err}");
    }

    // A name that parses as an id still names its dataset.
    let hex_name = "0123456789abcdef0123456789abcdef";
    assert!(hex_name.parse::<re_log_types::EntryId>().is_ok());
    let catalog = re_catalog_client::CatalogClient::new(
        registry.connection_handle(tools.origin().expect("connected")),
        runtime(),
    );
    let created = catalog
        .create_dataset(EntryName::new(hex_name).expect("valid name"), None)
        .await
        .expect("create");
    let info = tools
        .run(
            CatalogTool::GetDatasetInfo,
            args(json!({ "dataset": hex_name })),
        )
        .await
        .expect("found by name");
    assert_eq!(info["id"], created.details.id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_need_a_catalog() {
    let tools = CatalogTools::new(
        ConnectionRegistry::new_without_stored_credentials(),
        re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().expect("runtime"),
        None,
    );
    let err = tools
        .run(CatalogTool::ListEntries, Map::new())
        .await
        .expect_err("not connected");
    assert!(err.to_string().contains("catalog_connect"), "{err}");
}

#[test]
fn plain_http_urls_are_catalog_urls() {
    assert_eq!(
        CatalogTools::parse_url(" http://127.0.0.1:51234").expect("valid"),
        CatalogTools::parse_url("rerun+http://127.0.0.1:51234").expect("valid")
    );
    let err = CatalogTools::parse_url("ftp://example.com").expect_err("invalid");
    assert!(err.to_string().contains("URL: ftp://example.com"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn works_as_an_agent_toolbox() {
    use re_agent_tools::{AgentToolbox as _, ToolOutput};

    let (tools, _recordings, _storage) = connected_tools().await;

    let names: Vec<String> = tools
        .definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect();
    assert_eq!(
        names,
        CatalogTool::VARIANTS
            .iter()
            .map(|tool| tool.name().to_owned())
            .collect::<Vec<_>>()
    );
    assert!(!tools.requires_approval("catalog_list_entries", &Map::new()));
    assert!(tools.requires_approval("catalog_connect", &Map::new()));
    assert!(tools.requires_approval("catalog_nope", &Map::new()));

    let outputs = tools
        .call("catalog_list_entries", Map::new())
        .await
        .expect("entries");
    assert!(
        matches!(outputs.as_slice(), [ToolOutput::Text(text)] if text.contains(DATASET)),
        "{outputs:?}"
    );
    let err = tools
        .call("catalog_nope", Map::new())
        .await
        .expect_err("unknown tool");
    assert!(err.to_string().contains("catalog_nope"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn queries_a_dataset() {
    let (tools, _recordings, _storage) = connected_tools().await;

    let info = tools
        .run(
            CatalogTool::GetDatasetInfo,
            args(json!({ "dataset": DATASET })),
        )
        .await
        .expect("info");
    let timeline = info["timelines"][0]["name"].as_str().expect("a timeline");

    let rows = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({
                "dataset": DATASET,
                "index": timeline,
                "entity_paths": ["/points"],
                "max_rows": 2,
            })),
        )
        .await
        .expect("rows");
    assert_eq!(rows["rows"].as_array().expect("rows").len(), 2, "{rows}");
    assert!(rows["truncated"].is_string(), "{rows}");
    assert!(
        rows["columns"]
            .as_array()
            .expect("columns")
            .contains(&json!("rerun_segment_id")),
        "{rows}"
    );

    let per_segment = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({
                "dataset": DATASET,
                "index": timeline,
                "sql": "SELECT rerun_segment_id, count(*) AS n FROM dataset GROUP BY rerun_segment_id",
            })),
        )
        .await
        .expect("rows");
    assert_eq!(
        per_segment["rows"].as_array().expect("rows").len(),
        2,
        "{per_segment}"
    );
    assert!(per_segment.get("truncated").is_none(), "{per_segment}");
}

#[tokio::test(flavor = "multi_thread")]
async fn dataset_queries_apply_their_filters() {
    let (tools, _recordings, _storage) = connected_tools().await;

    // Without `index`: the dataset has one timeline, which becomes the index.
    let one_segment = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({
                "dataset": DATASET,
                "segments": ["segment_a"],
                "sql": "SELECT DISTINCT rerun_segment_id FROM dataset",
            })),
        )
        .await
        .expect("rows");
    assert_eq!(one_segment["rows"], json!([["segment_a"]]), "{one_segment}");

    let points_only = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({ "dataset": DATASET, "entity_paths": ["/points"], "max_rows": 1 })),
        )
        .await
        .expect("rows");
    let columns: Vec<&str> = points_only["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        columns.iter().any(|column| column.starts_with("/points")),
        "{columns:?}"
    );
    assert!(
        !columns.iter().any(|column| column.starts_with("/camera")),
        "{columns:?}"
    );

    let err = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({ "dataset": DATASET, "segments": ["segment_a", "nope"] })),
        )
        .await
        .expect_err("unknown segment");
    assert!(err.to_string().contains("\"nope\""), "{err}");

    let err = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({ "dataset": DATASET, "entity_paths": [] })),
        )
        .await
        .expect_err("no entities");
    assert!(err.to_string().contains("`entity_paths` is empty"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_sql_over_tables() {
    let (tools, _recordings, _storage) = connected_tools().await;

    let entries = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": "SELECT * FROM __entries" })),
        )
        .await
        .expect("rows");
    assert!(
        !entries["rows"].as_array().expect("rows").is_empty(),
        "{entries}"
    );

    let err = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": "SELECT 1 +" })),
        )
        .await
        .expect_err("invalid SQL");
    assert!(err.to_string().starts_with("Invalid query"), "{err}");

    let err = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": "SELECT * FROM nope" })),
        )
        .await
        .expect_err("no such table");
    assert!(err.to_string().contains("`nope`"), "{err}");

    let series = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": "SELECT * FROM generate_series(1, 3)" })),
        )
        .await
        .expect("table functions need no catalog table");
    assert_eq!(series["rows"].as_array().map(Vec::len), Some(3), "{series}");

    let err = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": "SELECT * FROM a.b" })),
        )
        .await
        .expect_err("an unquoted dotted name");
    assert!(err.to_string().contains("`\"a.b\"`"), "{err}");

    let tables: Vec<String> = (0..17).map(|i| format!("t{i}")).collect();
    let err = tools
        .run(
            CatalogTool::QueryTables,
            args(json!({ "sql": format!("SELECT * FROM {}", tables.join(", ")) })),
        )
        .await
        .expect_err("too many tables");
    assert!(err.to_string().contains("17 tables"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn dataset_queries_name_only_what_exists() {
    let (tools, _recordings, _storage) = connected_tools().await;

    let err = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({ "dataset": DATASET, "index": "nope" })),
        )
        .await
        .expect_err("no such timeline");
    assert!(err.to_string().contains("no timeline `nope`"), "{err}");

    let err = tools
        .run(
            CatalogTool::QueryDataset,
            args(json!({ "dataset": DATASET, "entity_paths": ["/points", "/nope"] })),
        )
        .await
        .expect_err("no such entity");
    assert!(err.to_string().contains("`/nope`"), "{err}");
}
