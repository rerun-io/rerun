//! Client-side coverage for assets: resolving the assets of a dataset, pulling them in alongside
//! the recording segment they belong to, and reusing them across the segments that share them.

use std::str::FromStr as _;
use std::time::Duration;

use arrow::array::RecordBatch;
use egui_kittest::Harness;
use futures::StreamExt as _;

use re_integration_test::{HarnessExt as _, TestServer, register_asset};
use re_log_channel::{DataSourceMessage, LogSource, RecordingOpenBehavior};
use re_log_encoding::RrdManifest;
use re_redap_client::{
    ApiError, ConnectionClient, ConnectionRegistry, ConnectionRegistryHandle, StreamingOptions,
};
use re_sdk::external::re_log_types::{EntryId, StoreId};
use re_sdk::external::re_tuid::Tuid;
use re_sdk_types::{ChunkId, SegmentId};
use re_viewer::external::re_entity_db::FetchStage;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const DATASET_NAME: &str = "my_dataset";
const DATASET_ID: &str = "187b552b95a5c2f73f37894708825ba5";
const RECORDING_ID: &str = "new_recording_id";

fn dataset_id() -> EntryId {
    Tuid::from_str(DATASET_ID)
        .expect("Failed to parse TUID")
        .into()
}

fn origin(server: &TestServer) -> re_uri::Origin {
    re_uri::Origin {
        scheme: re_uri::Scheme::RerunHttp,
        host: re_uri::external::url::Host::Domain("localhost".to_owned()),
        port: server.port(),
    }
}

fn segment_uri(server: &TestServer, segment_id: SegmentId) -> re_uri::DatasetSegmentUri {
    re_uri::DatasetSegmentUri {
        origin: origin(server),
        dataset_id: Tuid::from_str(DATASET_ID).expect("Failed to parse TUID"),
        segment_id,
        fragment: re_uri::Fragment::default(),
    }
}

/// The asset dataset that a recording dataset implicitly gets on creation.
async fn asset_dataset(client: &mut ConnectionClient) -> EntryId {
    client
        .read_dataset_entry(dataset_id())
        .await
        .expect("Failed to read dataset entry")
        .dataset_details
        .asset_dataset
        .expect("recording datasets get an implicit asset dataset")
}

/// The `FetchChunks` input covering every chunk of a segment, taken from its manifest.
async fn chunk_fetch_batch(
    client: &mut ConnectionClient,
    dataset: EntryId,
    segment_id: &SegmentId,
) -> RecordBatch {
    let raw_manifest = client
        .get_rrd_manifest(dataset, segment_id.clone())
        .await
        .expect("Failed to get the segment's manifest");

    RrdManifest::try_new(&raw_manifest)
        .expect("Failed to parse the segment's manifest")
        .chunk_fetcher_rb()
        .clone()
}

/// Fetches the chunks described by `batch` and returns their ids, sorted.
async fn fetch_chunk_ids(
    client: &mut ConnectionClient,
    batch: &RecordBatch,
) -> Result<Vec<ChunkId>, ApiError> {
    let stream = client.fetch_segment_chunks_by_id(batch).await?;
    let mut stream = re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(stream);

    let mut chunk_ids = Vec::new();
    while let Some(chunks) = stream.next().await {
        for (chunk, _segment_id) in chunks? {
            chunk_ids.push(chunk.id());
        }
    }
    chunk_ids.sort();

    Ok(chunk_ids)
}

/// A dataset gets its asset dataset on creation, so before anything is registered the client
/// reports that asset dataset with no segments in it. `None` is reserved for datasets created
/// before asset datasets existed, which have no asset dataset at all.
#[tokio::test(flavor = "multi_thread")]
async fn get_assets_for_segment_returns_no_segments_before_any_asset_is_registered() {
    let (server, _) = TestServer::spawn()
        .await
        .with_named_test_data(DATASET_NAME, DATASET_ID, RECORDING_ID)
        .await;
    let mut client = server.client().await.expect("Failed to connect to server");

    let asset_dataset = asset_dataset(&mut client).await;

    let assets = client
        .get_assets_for_segment(dataset_id())
        .await
        .expect("Failed to get assets");

    assert_eq!(assets, Some((asset_dataset, vec![])));
}

/// Once an asset is registered, the client reports the dataset's asset dataset along with every
/// asset segment in it.
#[tokio::test(flavor = "multi_thread")]
async fn get_assets_for_segment_returns_the_registered_assets() {
    let (server, _) = TestServer::spawn()
        .await
        .with_named_test_data(DATASET_NAME, DATASET_ID, RECORDING_ID)
        .await;
    let mut client = server.client().await.expect("Failed to connect to server");

    let asset_dataset = asset_dataset(&mut client).await;

    let mut expected_segments = Vec::new();
    for recording_id in ["robot_urdf", "warehouse_mesh"] {
        expected_segments.push(
            register_asset(&mut client, asset_dataset, recording_id)
                .await
                .expect("Failed to register asset"),
        );
    }
    expected_segments.sort();

    let (entry, mut segments) = client
        .get_assets_for_segment(dataset_id())
        .await
        .expect("Failed to get assets")
        .expect("the dataset should have assets");
    segments.sort();

    assert_eq!(entry, asset_dataset);
    assert_eq!(segments, expected_segments);
}

/// Streaming a segment also pulls in the manifests of the dataset's assets, addressed to the
/// recording's store so the asset data lands in the same store as the recording.
#[tokio::test(flavor = "multi_thread")]
async fn streaming_a_segment_delivers_its_asset_manifests() {
    let (server, segment_id) = TestServer::spawn()
        .await
        .with_named_test_data(DATASET_NAME, DATASET_ID, RECORDING_ID)
        .await;
    let mut client = server.client().await.expect("Failed to connect to server");

    let asset_dataset = asset_dataset(&mut client).await;
    let asset_segment_id = register_asset(&mut client, asset_dataset, "robot_urdf")
        .await
        .expect("Failed to register asset");

    let uri = segment_uri(&server, segment_id);
    let recording_store_id = uri.store_id();

    let (tx, rx) = re_log_channel::log_channel(LogSource::RedapGrpcStream {
        uri: uri.clone(),
        open_behavior: RecordingOpenBehavior::Background,
        table_blueprint: None,
    });

    re_redap_client::stream_blueprint_and_segment_from_server(
        client,
        tx,
        uri,
        StreamingOptions::default(),
    )
    .await
    .expect("Failed to stream segment");

    let mut messages: Vec<DataSourceMessage> = Vec::new();
    while let Ok(message) = rx.try_recv() {
        if let Some(data) = message.into_data() {
            messages.push(data);
        }
    }

    // Where each manifest was addressed to, and which store it actually describes.
    let manifests: Vec<(StoreId, StoreId)> = messages
        .iter()
        .filter_map(|message| match message {
            DataSourceMessage::RrdManifest(store_id, manifest) => {
                Some((store_id.clone(), manifest.store_id().clone()))
            }
            _ => None,
        })
        .collect();

    assert!(
        manifests
            .iter()
            .all(|(addressed_to, _)| *addressed_to == recording_store_id),
        "every manifest should be addressed to the recording's store, got {manifests:?}"
    );

    let sources: Vec<SegmentId> = manifests
        .iter()
        .map(|(_, describes)| SegmentId::from(describes.recording_id()))
        .collect();
    assert!(
        sources.contains(&asset_segment_id),
        "the asset's manifest should have been delivered, got {sources:?}"
    );
    assert!(
        sources.contains(&SegmentId::from(recording_store_id.recording_id())),
        "the recording's own manifest should have been delivered, got {sources:?}"
    );

    let completions = messages
        .iter()
        .filter(|message| matches!(message, DataSourceMessage::RrdManifestComplete(_)))
        .count();
    assert_eq!(
        completions, 1,
        "the recording's store should be told exactly once that its manifests are all in"
    );
}

/// An asset's chunks are cached on the connection the first time they are downloaded, so asking
/// for them again is served locally. The recording's own chunks are not cached, since no other
/// segment ever needs them.
#[tokio::test(flavor = "multi_thread")]
async fn asset_chunks_are_only_downloaded_once() {
    let (server, segment_id) = TestServer::spawn()
        .await
        .with_named_test_data(DATASET_NAME, DATASET_ID, RECORDING_ID)
        .await;

    // Every client handed out by one registry shares a connection, and with it a chunk cache.
    let registry = ConnectionRegistry::new_without_stored_credentials();
    let mut client = registry
        .client(origin(&server))
        .await
        .expect("Failed to connect to server");

    let asset_dataset = asset_dataset(&mut client).await;
    let asset_segment_id = register_asset(&mut client, asset_dataset, "robot_urdf")
        .await
        .expect("Failed to register asset");

    let uri = segment_uri(&server, segment_id.clone());
    let (tx, _rx) = re_log_channel::log_channel(LogSource::RedapGrpcStream {
        uri: uri.clone(),
        open_behavior: RecordingOpenBehavior::Background,
        table_blueprint: None,
    });

    // Streaming the segment delivers the asset's manifest, which is what marks the asset's chunks
    // as cacheable.
    re_redap_client::stream_blueprint_and_segment_from_server(
        client.clone(),
        tx,
        uri,
        StreamingOptions::default(),
    )
    .await
    .expect("Failed to stream segment");

    let asset_chunks = chunk_fetch_batch(&mut client, asset_dataset, &asset_segment_id).await;
    let own_chunks = chunk_fetch_batch(&mut client, dataset_id(), &segment_id).await;

    let downloaded_asset_chunks = fetch_chunk_ids(&mut client, &asset_chunks)
        .await
        .expect("Failed to fetch the asset's chunks");
    assert!(
        !downloaded_asset_chunks.is_empty(),
        "the asset should have chunks to fetch"
    );
    fetch_chunk_ids(&mut client, &own_chunks)
        .await
        .expect("Failed to fetch the recording's own chunks");

    // The server refuses to serve chunks from here on, so whatever still arrives came from the
    // cache.
    server.injected_errors().inject("FetchChunks");

    let cached_asset_chunks = fetch_chunk_ids(&mut client, &asset_chunks)
        .await
        .expect("the asset's chunks should be served from the cache");
    assert_eq!(
        cached_asset_chunks, downloaded_asset_chunks,
        "asking for the asset's chunks a second time should not reach the server"
    );

    assert!(
        fetch_chunk_ids(&mut client, &own_chunks).await.is_err(),
        "the recording's own chunks are not cached, so they should still reach the server"
    );
}

/// Two segments of the same dataset share one asset. The viewer downloads the asset while opening
/// the first segment and keeps it cached, so the second segment gets the same data without
/// downloading it again.
#[tokio::test(flavor = "multi_thread")]
async fn the_viewer_shares_asset_chunks_between_segments() {
    let (server, segment_ids) = TestServer::spawn()
        .await
        .with_static_preview_data(DATASET_NAME, DATASET_ID, RECORDING_ID, 2)
        .await;
    let [first_segment, second_segment]: [SegmentId; 2] = segment_ids
        .try_into()
        .expect("two recordings were registered");

    let mut client = server.client().await.expect("Failed to connect to server");
    let asset_dataset = asset_dataset(&mut client).await;
    let asset_segment_id = register_asset(&mut client, asset_dataset, "robot_urdf")
        .await
        .expect("Failed to register asset");

    let first_uri = segment_uri(&server, first_segment);
    let second_uri = segment_uri(&server, second_segment);

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(first_uri.to_string()),
        app_options_editor: Some(Box::new(|app_options| {
            app_options.max_fetch_stage = FetchStage::Everything;
        })),
        ..Default::default()
    });

    step_until_asset_chunks_are_loaded(&mut harness, &first_uri, &asset_segment_id);

    harness.state().open_url_or_file(&second_uri.to_string());
    step_until_asset_chunks_are_loaded(&mut harness, &second_uri, &asset_segment_id);

    // The viewer's connection is still holding on to the asset's chunks, so any further segment
    // of this dataset would be served from the cache too.
    let registry: ConnectionRegistryHandle =
        harness.run_with_app_context(|ctx| ctx.connection_registry.clone());
    server.injected_errors().inject("FetchChunks");

    let mut client = registry
        .client(origin(&server))
        .await
        .expect("Failed to connect to server");
    let asset_chunks = chunk_fetch_batch(&mut client, asset_dataset, &asset_segment_id).await;
    let cached_asset_chunks = fetch_chunk_ids(&mut client, &asset_chunks)
        .await
        .expect("the viewer should still have the asset's chunks cached");
    assert!(
        !cached_asset_chunks.is_empty(),
        "the asset should have chunks in the cache"
    );
}

/// Steps the viewer until the store behind `uri` holds chunks that came from `asset_segment_id`.
fn step_until_asset_chunks_are_loaded(
    harness: &mut Harness<'static, re_viewer::App>,
    uri: &re_uri::DatasetSegmentUri,
    asset_segment_id: &SegmentId,
) {
    viewer_test_utils::step_until(
        "The asset's chunks are loaded into the segment's store",
        harness,
        |harness| {
            let uri = uri.clone();
            let asset_segment_id = asset_segment_id.clone();
            harness.run_with_app_context(move |ctx| {
                ctx.storage_context
                    .hub
                    .find_recording_by_uri(&uri)
                    .is_some_and(|db| {
                        let engine = db.storage_engine();
                        let store = engine.store();
                        store.iter_physical_chunks().any(|chunk| {
                            store
                                .find_source_segments(&chunk.id())
                                .contains(&asset_segment_id)
                        })
                    })
            })
        },
        Duration::from_millis(10),
        Duration::from_secs(10),
    );
}
