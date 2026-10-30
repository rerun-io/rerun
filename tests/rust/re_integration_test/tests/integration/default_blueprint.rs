//! A default blueprint loads on a best-effort basis. If it fails to load, the viewer gets no
//! activation or registration for it, but the recording or table stream still completes.

use std::str::FromStr as _;
use std::time::Duration;

use re_catalog_client::CatalogClient;
use re_integration_test::{TestServer, file_url};
use re_log_channel::{
    BlueprintTarget, DataSourceMessage, DefaultBlueprintRegistration, LogReceiver, LogSource,
    RecordingOpenBehavior, SmartMessagePayload,
};
use re_protos::cloud::v1alpha1::ext::{DataSource, DatasetDetails};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{ApiErrorKind, ApiResult, StreamingOptions};
use re_sdk::RecordingStreamBuilder;
use re_sdk::external::re_log_msg::{BlueprintActivationCommand, LogMsg};
use re_sdk::external::re_log_types::{EntryId, StoreId, StoreKind};
use re_sdk::external::re_tuid::Tuid;
use re_sdk_types::SegmentId;

const DATASET_NAME: &str = "blueprint_dataset";
const DATASET_ID: &str = "187b552b95a5c2f73f37894708825ba5";
const RECORDING_ID: &str = "blueprint_recording";

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

fn segment_uri(server: &TestServer, segment_id: SegmentId) -> re_uri::DatasetUri {
    re_uri::DatasetUri {
        origin: origin(server),
        dataset_id: Tuid::from_str(DATASET_ID).expect("Failed to parse TUID"),
        resource: re_uri::DatasetResource::Segments,
        segment_id: Some(segment_id),
        fragment: re_uri::Fragment::default(),
    }
}

/// How the dataset's default blueprint behaves when loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DefaultBlueprint {
    /// A registered blueprint that loads.
    Loads,

    /// A segment id with no registered segment, so its manifest fails to open.
    Missing,

    /// A registered blueprint whose chunk download fails after its manifest arrives.
    FailsToDownload,
}

/// Spawns a server with one recording segment in the dataset, and returns that segment.
async fn spawn_server() -> (TestServer, SegmentId) {
    TestServer::spawn()
        .await
        .with_named_test_data(DATASET_NAME, DATASET_ID, RECORDING_ID)
        .await
}

/// Writes a one-view `.rbl`, so the registered blueprint has chunks to download.
fn blueprint_rbl() -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("Failed to create the blueprint file");
    let stream = RecordingStreamBuilder::new("rerun_example_integration_test")
        .blueprint()
        .save(file.path())
        .expect("Failed to create the blueprint stream");
    re_sdk::blueprint::Blueprint::new(re_sdk::blueprint::Spatial3DView::new("3D"))
        .send(&stream, re_sdk::blueprint::BlueprintActivation::default())
        .expect("Failed to log the blueprint");
    stream
        .flush_with_timeout(Duration::from_mins(1))
        .expect("Failed to flush the blueprint");
    file
}

/// Registers a blueprint with `blueprint_dataset` and returns its segment id.
async fn register_blueprint(
    catalog_client: &CatalogClient,
    blueprint_dataset: EntryId,
) -> SegmentId {
    let rbl = blueprint_rbl();
    let data_source = DataSource::new_rrd(
        file_url(rbl.path()).expect("Failed to build the blueprint url"),
        None,
    )
    .expect("Failed to build the blueprint data source");
    catalog_client
        .register_with_dataset(
            blueprint_dataset,
            vec![data_source],
            IfDuplicateBehavior::Error,
        )
        .await
        .expect("Failed to register the blueprint")
        .wait(Duration::from_secs(10))
        .await
        .expect("Failed to finish the blueprint registration")
        .into_iter()
        .next()
        .expect("the registration returns the blueprint segment")
}

/// Makes a blueprint segment the dataset's default, set up to behave as `default_blueprint` says.
///
/// Returns the blueprint dataset and segment.
async fn set_up_default_blueprint(
    server: &TestServer,
    default_blueprint: DefaultBlueprint,
) -> (EntryId, SegmentId) {
    let catalog_client = server.catalog_client();
    let mut client = catalog_client
        .client()
        .await
        .expect("Failed to connect to server");
    let entry = client
        .read_dataset_entry(dataset_id())
        .await
        .expect("Failed to read the dataset entry");
    let blueprint_dataset = entry
        .dataset_details
        .blueprint_dataset
        .expect("datasets get an implicit blueprint dataset");

    let blueprint_segment = match default_blueprint {
        DefaultBlueprint::Loads | DefaultBlueprint::FailsToDownload => {
            register_blueprint(&catalog_client, blueprint_dataset).await
        }
        DefaultBlueprint::Missing => SegmentId::from("missing_blueprint"),
    };

    client
        .update_dataset_entry(
            dataset_id(),
            DatasetDetails {
                default_blueprint_segment: Some(blueprint_segment.clone()),
                ..entry.dataset_details
            },
        )
        .await
        .expect("Failed to set the default blueprint");

    // Blueprints are always downloaded in full, while the recording loads its chunks on demand,
    // so only the blueprint sends `FetchChunks`.
    if default_blueprint == DefaultBlueprint::FailsToDownload {
        server.injected_errors().inject("FetchChunks");
    }

    (blueprint_dataset, blueprint_segment)
}

/// Every payload left in the channel, in order. Call only after every sender is gone.
fn drain(rx: &LogReceiver) -> Vec<SmartMessagePayload> {
    let mut payloads = Vec::new();
    while let Ok(message) = rx.try_recv() {
        payloads.push(message.payload);
    }
    payloads
}

/// Streams a recording segment with the dataset's default blueprint.
async fn stream_recording(
    server: &TestServer,
    segment_id: SegmentId,
) -> (ApiResult, Vec<SmartMessagePayload>) {
    let uri = segment_uri(server, segment_id);
    let (tx, rx) = re_log_channel::log_channel(LogSource::RedapGrpcStream {
        uri: uri.clone(),
        open_behavior: RecordingOpenBehavior::Background,
    });
    let client = server.client().await.expect("Failed to connect to server");

    let result = re_redap_client::stream_blueprint_and_segment_from_server(
        client,
        tx,
        uri,
        StreamingOptions::default(),
    )
    .await;

    (result, drain(&rx))
}

fn data(payloads: &[SmartMessagePayload]) -> impl Iterator<Item = &DataSourceMessage> {
    payloads.iter().filter_map(|payload| match payload {
        SmartMessagePayload::Msg(message) => Some(message),
        SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => None,
    })
}

fn activation_commands(payloads: &[SmartMessagePayload]) -> Vec<&BlueprintActivationCommand> {
    data(payloads)
        .filter_map(|message| match message {
            DataSourceMessage::LogMsg(LogMsg::BlueprintActivationCommand(command)) => Some(command),
            _ => None,
        })
        .collect()
}

fn registrations(payloads: &[SmartMessagePayload]) -> Vec<&DefaultBlueprintRegistration> {
    data(payloads)
        .filter_map(|message| match message {
            DataSourceMessage::DefaultBlueprintRegistration(registration) => Some(registration),
            _ => None,
        })
        .collect()
}

/// The stores of `kind` that received a manifest part.
fn manifests(payloads: &[SmartMessagePayload], kind: StoreKind) -> Vec<&StoreId> {
    data(payloads)
        .filter_map(|message| match message {
            DataSourceMessage::RrdManifest(store_id, _) if store_id.kind() == kind => {
                Some(store_id)
            }
            _ => None,
        })
        .collect()
}

/// The stores of `kind` that were told their manifest is complete.
fn manifest_completions(payloads: &[SmartMessagePayload], kind: StoreKind) -> Vec<&StoreId> {
    data(payloads)
        .filter_map(|message| match message {
            DataSourceMessage::RrdManifestComplete(store_id) if store_id.kind() == kind => {
                Some(store_id)
            }
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_default_blueprint_that_loads_is_activated() {
    let (server, segment_id) = spawn_server().await;
    set_up_default_blueprint(&server, DefaultBlueprint::Loads).await;
    let recording_store_id = segment_uri(&server, segment_id.clone())
        .store_id()
        .expect("the uri names a segment");

    let (result, payloads) = stream_recording(&server, segment_id).await;

    result.expect("Failed to stream the recording");
    let blueprint_completions = manifest_completions(&payloads, StoreKind::Blueprint);
    assert_eq!(
        blueprint_completions.len(),
        1,
        "the blueprint manifest should complete once"
    );
    let blueprint_store_id = blueprint_completions[0];
    assert_eq!(
        blueprint_store_id.application_id(),
        recording_store_id.application_id(),
        "the blueprint should belong to the recording's application"
    );
    assert_eq!(
        activation_commands(&payloads),
        vec![&BlueprintActivationCommand {
            blueprint_id: blueprint_store_id.clone(),
            make_active: false,
            make_default: true,
        }]
    );
    assert_eq!(
        manifest_completions(&payloads, StoreKind::Recording),
        vec![&recording_store_id]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_default_blueprint_that_fails_to_load_is_skipped() {
    for default_blueprint in [DefaultBlueprint::Missing, DefaultBlueprint::FailsToDownload] {
        let (server, segment_id) = spawn_server().await;
        set_up_default_blueprint(&server, default_blueprint).await;
        let recording_store_id = segment_uri(&server, segment_id.clone())
            .store_id()
            .expect("the uri names a segment");

        let (result, payloads) = stream_recording(&server, segment_id).await;

        result.unwrap_or_else(|err| {
            panic!("{default_blueprint:?}: the recording should still load, got {err}")
        });
        // A blueprint manifest shows the failure came after the manifest stream opened.
        assert_eq!(
            manifests(&payloads, StoreKind::Blueprint).is_empty(),
            default_blueprint == DefaultBlueprint::Missing,
            "{default_blueprint:?}: unexpected blueprint manifest delivery"
        );
        assert!(
            activation_commands(&payloads).is_empty(),
            "{default_blueprint:?}: a blueprint that failed to load must not be activated"
        );
        assert_eq!(
            manifest_completions(&payloads, StoreKind::Recording),
            vec![&recording_store_id],
            "{default_blueprint:?}"
        );
    }
}

/// A table blueprint stream registers the blueprint only if it loads, and always ends with a
/// successful quit marker.
#[tokio::test(flavor = "multi_thread")]
async fn a_table_blueprint_stream_completes_whether_or_not_the_blueprint_loads() {
    for default_blueprint in [
        DefaultBlueprint::Loads,
        DefaultBlueprint::Missing,
        DefaultBlueprint::FailsToDownload,
    ] {
        let (server, _) = spawn_server().await;
        let (blueprint_dataset, blueprint_segment) =
            set_up_default_blueprint(&server, default_blueprint).await;
        let blueprint_store_id =
            StoreId::random(StoreKind::Blueprint, "rerun_example_table_blueprint");
        let table_ref = re_uri::TableReference::RedapEntry {
            origin: origin(&server),
            entry_id: dataset_id(),
        };
        let (tx, rx) = re_redap_client::table_blueprint_log_channel(
            origin(&server),
            blueprint_dataset,
            &blueprint_segment,
        );
        let client = server.client().await.expect("Failed to connect to server");

        re_redap_client::stream_table_blueprint_segment_from_server(
            client,
            tx,
            blueprint_store_id.clone(),
            table_ref.clone(),
            blueprint_dataset,
            blueprint_segment,
        )
        .await
        .unwrap_or_else(|err| {
            panic!("{default_blueprint:?}: the stream should end cleanly, got {err}")
        });
        let payloads = drain(&rx);

        let registrations = registrations(&payloads);
        if default_blueprint == DefaultBlueprint::Loads {
            let [registration] = registrations.as_slice() else {
                panic!("expected exactly one registration, got {registrations:?}");
            };
            assert_eq!(registration.blueprint_id, blueprint_store_id);
            assert!(
                matches!(&registration.target, BlueprintTarget::Table(target) if *target == table_ref),
                "unexpected registration target {:?}",
                registration.target
            );
        } else {
            assert!(
                registrations.is_empty(),
                "{default_blueprint:?}: a blueprint that failed to load must not be registered"
            );
        }
        assert!(
            activation_commands(&payloads).is_empty(),
            "{default_blueprint:?}: a table blueprint is never activated"
        );
        let quit_count = payloads
            .iter()
            .filter(|payload| matches!(payload, SmartMessagePayload::Quit(_)))
            .count();
        assert_eq!(
            quit_count, 1,
            "{default_blueprint:?}: expected one quit marker"
        );
        assert!(
            matches!(payloads.last(), Some(SmartMessagePayload::Quit(None))),
            "{default_blueprint:?}: the stream should end with a successful quit marker, got {payloads:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_disconnected_receiver_ends_both_streams_cleanly() {
    let (server, segment_id) = spawn_server().await;
    let (blueprint_dataset, blueprint_segment) =
        set_up_default_blueprint(&server, DefaultBlueprint::Loads).await;

    let uri = segment_uri(&server, segment_id);
    let (tx, rx) = re_log_channel::log_channel(LogSource::RedapGrpcStream {
        uri: uri.clone(),
        open_behavior: RecordingOpenBehavior::Background,
    });
    drop(rx);
    re_redap_client::stream_blueprint_and_segment_from_server(
        server.client().await.expect("Failed to connect to server"),
        tx,
        uri,
        StreamingOptions::default(),
    )
    .await
    .expect("a disconnected receiver should end the recording stream cleanly");

    let (tx, rx) = re_redap_client::table_blueprint_log_channel(
        origin(&server),
        blueprint_dataset,
        &blueprint_segment,
    );
    drop(rx);
    re_redap_client::stream_table_blueprint_segment_from_server(
        server.client().await.expect("Failed to connect to server"),
        tx,
        StoreId::random(StoreKind::Blueprint, "rerun_example_table_blueprint"),
        re_uri::TableReference::RedapEntry {
            origin: origin(&server),
            entry_id: dataset_id(),
        },
        blueprint_dataset,
        blueprint_segment,
    )
    .await
    .expect("a disconnected receiver should end the table blueprint stream cleanly");
}

/// Only the blueprint is best effort: a recording that fails to load is still an error.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_recording_segment_is_an_error() {
    let (server, _) = spawn_server().await;
    set_up_default_blueprint(&server, DefaultBlueprint::Loads).await;

    let (result, _) = stream_recording(&server, SegmentId::from("missing_recording")).await;

    let err = result.expect_err("a missing recording segment should fail the stream");
    assert_eq!(err.kind, ApiErrorKind::NotFound, "unexpected error: {err}");
}
