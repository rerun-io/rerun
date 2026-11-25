//! The blueprint dropdown in the top bar, for a recording opened from a catalog dataset.

use std::time::Duration;

use egui_kittest::kittest::Queryable as _;
use re_catalog_client::{BlueprintKind, BlueprintName, CatalogClient};
use re_integration_test::{TestServer, ViewerHarnessExt as _, file_url, spatial_3d_blueprint_rbl};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_sdk::external::re_log_types::EntryId;
use re_sdk_types::SegmentId;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

type ViewerHarness = egui_kittest::Harness<'static, re_viewer::App>;

/// A server with one dataset that has a recording and two recording blueprints.
struct Setup {
    server: TestServer,

    /// The dataset that holds the blueprints of the dataset.
    blueprint_dataset: EntryId,

    /// The dataset's default blueprint, named "First".
    first: SegmentId,

    /// The blueprint named "Second".
    second: SegmentId,

    /// The URL of the recording.
    recording_url: String,
}

impl Setup {
    async fn new() -> Self {
        let (server, recording_segment) = TestServer::spawn().await.with_test_data().await;

        let catalog_client = server.catalog_client();
        let datasets = catalog_client
            .find_entries(EntryFilter::default().with_entry_kinds([EntryKind::Dataset]))
            .await
            .expect("Failed to list the datasets");
        let [dataset] = datasets.as_slice() else {
            panic!("expected one dataset, got {}", datasets.len());
        };
        let dataset = dataset.id;

        let blueprint_dataset = catalog_client
            .client()
            .await
            .expect("Failed to connect to server")
            .read_dataset_entry(dataset)
            .await
            .expect("Failed to read the dataset entry")
            .dataset_details
            .blueprint_dataset
            .expect("datasets get an implicit blueprint dataset");

        let first = register_recording_blueprint(&catalog_client, blueprint_dataset, "First").await;
        let second =
            register_recording_blueprint(&catalog_client, blueprint_dataset, "Second").await;
        catalog_client
            .set_dataset_default_blueprint(dataset, BlueprintKind::Recording, Some(first.clone()))
            .await
            .expect("Failed to set the default blueprint");

        let recording_url = re_uri::DatasetUri {
            origin: re_uri::Origin {
                scheme: re_uri::Scheme::RerunHttp,
                host: re_uri::external::url::Host::Domain("localhost".to_owned()),
                port: server.port(),
            },
            dataset_id: dataset.id,
            resource: re_uri::DatasetResource::Segments,
            segment_id: Some(recording_segment),
            fragment: re_uri::Fragment::default(),
        }
        .to_string();

        Self {
            server,
            blueprint_dataset,
            first,
            second,
            recording_url,
        }
    }

    /// Opens the recording in a new viewer, and waits until a copy of the dataset's default
    /// blueprint is active and the blueprint dropdown shows its name.
    fn open_recording(&self) -> ViewerHarness {
        let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
            startup_url: Some(self.recording_url.clone()),
            ..Default::default()
        });

        harness.step_until("the default blueprint is active", |harness| {
            active_dataset_blueprint(harness).as_ref() == Some(&self.first)
                && harness.has_label("First")
        });

        harness
    }
}

/// Registers a recording blueprint named `name` with one view named "`name` view", and returns
/// its segment in the blueprint dataset.
async fn register_recording_blueprint(
    catalog_client: &CatalogClient,
    blueprint_dataset: EntryId,
    name: &str,
) -> SegmentId {
    let rbl =
        spatial_3d_blueprint_rbl(&format!("{name} view")).expect("Failed to write the blueprint");
    catalog_client
        .register_blueprint(
            blueprint_dataset,
            file_url(rbl.path()).expect("Failed to build the blueprint url"),
            Some(&BlueprintName::try_new(name).expect("the name is not empty")),
            BlueprintKind::Recording,
            None,
            Duration::from_secs(10),
        )
        .await
        .expect("Failed to register the blueprint")
        .segment_id
}

/// The segment of the dataset blueprint that the active blueprint of the open recording is a copy
/// of.
fn active_dataset_blueprint(harness: &ViewerHarness) -> Option<SegmentId> {
    let app = harness.state();
    let active_id = app
        .store_hub()
        .active_blueprint_id_for_app(app.route().app_id()?)?;
    app.store_hub().copied_dataset_blueprint(active_id)
}

/// A recording of a dataset with two blueprints is open with the dataset's default blueprint. The
/// user picks the other blueprint in the blueprint dropdown, and then the heuristic blueprint.
/// Each pick becomes the active blueprint, and the dropdown shows its name.
#[tokio::test(flavor = "multi_thread")]
async fn picking_a_row_makes_that_blueprint_active() {
    let setup = Setup::new().await;
    let mut harness = setup.open_recording();
    assert!(
        harness.has_label("First view"),
        "the view of the default blueprint should be shown"
    );

    harness.get_by_label("First").click();
    harness.step_until("the dropdown lists the blueprints", |harness| {
        harness.has_label("Second")
    });
    harness.get_by_label("Second").click();
    harness.step_until("the picked blueprint is active", |harness| {
        active_dataset_blueprint(harness).as_ref() == Some(&setup.second)
            && harness.has_label("Second view")
    });
    assert!(!harness.has_label("First"), "the dropdown should be closed");
    assert_eq!(
        harness.query_all_by_label("Second").count(),
        1,
        "the dropdown button should show the picked blueprint"
    );

    harness.get_by_label("Second").click();
    harness.step_until("the dropdown lists the blueprints", |harness| {
        harness.has_label("Heuristic")
    });
    harness.get_by_label("Heuristic").click();
    harness.step_until("the heuristic blueprint is active", |harness| {
        let app = harness.state();
        app.route()
            .app_id()
            .and_then(|app_id| app.store_hub().active_blueprint_id_for_app(app_id))
            .is_some_and(|active_id| active_id.is_heuristic_blueprint())
            && !harness.has_label("Second")
    });
    assert_eq!(
        harness.query_all_by_label("Heuristic").count(),
        1,
        "the dropdown button should show the heuristic blueprint"
    );
}

/// The user opens and closes the blueprint dropdown, and then a blueprint is registered for the
/// dataset. The user opens the blueprint dropdown again, and the new blueprint is listed.
#[tokio::test(flavor = "multi_thread")]
async fn opening_the_dropdown_lists_the_blueprints_again() {
    let setup = Setup::new().await;
    let mut harness = setup.open_recording();

    harness.get_by_label("First").click();
    harness.step_until("the dropdown lists the blueprints", |harness| {
        harness.has_label("Second")
    });
    harness.key_press(egui::Key::Escape);
    harness.step_until("the dropdown is closed", |harness| {
        !harness.has_label("Second")
    });

    register_recording_blueprint(
        &setup.server.catalog_client(),
        setup.blueprint_dataset,
        "Third",
    )
    .await;

    harness.get_by_label("First").click();
    harness.step_until("the dropdown lists the new blueprint", |harness| {
        harness.has_label("Third")
    });
    assert!(
        harness.has_label("Second"),
        "the blueprints listed before should stay"
    );
}
