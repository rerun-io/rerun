use std::error::Error;
use std::str::FromStr as _;
use std::time::Duration;

use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::{DataSource, TableDetails, TableEntry};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionHandle;
use re_sdk::external::re_log_types::EntryId;
use re_sdk::external::re_tuid;
use re_sdk::time::TimeType;
use re_sdk::{RecordingStreamBuilder, TimeCell};
use re_sdk_types::SegmentId;
use re_viewer::external::re_sdk_types::{archetypes, components::Color};

pub async fn load_test_data(connection: &ConnectionHandle) -> Result<SegmentId, Box<dyn Error>> {
    load_test_data_with_name(
        connection,
        "my_dataset",
        "187b552b95a5c2f73f37894708825ba5",
        "new_recording_id",
    )
    .await
}

pub async fn load_test_data_with_name(
    connection: &ConnectionHandle,
    dataset_name: &str,
    dataset_id_str: &str,
    recording_id: &str,
) -> Result<SegmentId, Box<dyn Error>> {
    let mut client = connection.client().await?;
    let path = recording_rrd(recording_id, |stream| {
        for x in 0..20 {
            stream.set_time("test_time", TimeCell::new(TimeType::Sequence, x));
            stream
                .log(
                    "test_entity",
                    &archetypes::Points3D::new([(x as f32, 0.0, 0.0)]),
                )
                .expect("Failed to log points 3D");
        }
    })?;

    // Make sure that we have an entries table.
    let entries_table = client
        .find_entries(EntryFilter::default().with_entry_kinds([EntryKind::Table]))
        .await?;
    assert_eq!(entries_table.len(), 1);
    assert_eq!(entries_table[0].name, re_protos::EntryName::entries_table());
    assert_eq!(entries_table[0].kind, EntryKind::Table);

    let segment_ids =
        register_rrds(connection, dataset_name, dataset_id_str, &[path.path()]).await?;
    Ok(segment_ids
        .into_iter()
        .next()
        .expect("We registered exactly one recording"))
}

/// Logs `count` recordings with static `Points3D` and registers them in a fresh dataset, one
/// segment per recording. Returns the segment ids in registration order.
///
/// Each recording uses a different point color so the segment previews look distinct. The data
/// is time-invariant, so a preview renders identically at every point on its looping preview
/// timeline. That keeps preview snapshots stable.
pub async fn load_static_preview_data(
    connection: &ConnectionHandle,
    dataset_name: &str,
    dataset_id_str: &str,
    recording_id_prefix: &str,
    count: usize,
) -> Result<Vec<SegmentId>, Box<dyn Error>> {
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let color = preview_segment_color(i);
        let path = recording_rrd(&format!("{recording_id_prefix}_{i}"), |stream| {
            stream
                .log_static(
                    "test_entity",
                    &archetypes::Points3D::new([
                        (0.0, 0.0, 0.0),
                        (1.0, 0.0, 0.0),
                        (0.0, 1.0, 0.0),
                        (0.0, 0.0, 1.0),
                    ])
                    .with_radii([0.3])
                    .with_colors([color]),
                )
                .expect("Failed to log static points 3D");
        })?;
        paths.push(path);
    }

    let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.path()).collect();
    register_rrds(connection, dataset_name, dataset_id_str, &path_refs).await
}

/// A distinct color for the segment at `index`, cycling through a small fixed palette.
fn preview_segment_color(index: usize) -> Color {
    const PALETTE: [(u8, u8, u8); 6] = [
        (230, 80, 80),
        (80, 200, 120),
        (80, 140, 230),
        (230, 200, 80),
        (190, 100, 220),
        (90, 210, 210),
    ];
    let (r, g, b) = PALETTE[index % PALETTE.len()];
    Color::from_rgb(r, g, b)
}

/// Build an `.rrd` file from a recording, running `log_data` to populate it.
fn recording_rrd(
    recording_id: &str,
    log_data: impl FnOnce(&re_sdk::RecordingStream),
) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
    let path = tempfile::NamedTempFile::new()?;
    let stream = RecordingStreamBuilder::new("rerun_example_integration_test")
        .recording_id(recording_id)
        // The built-in properties carry the recording start time, which the viewer would show as
        // an ever-changing timestamp in snapshots.
        .send_properties(false)
        .save(path.path())?;

    log_data(&stream);

    stream.flush_with_timeout(Duration::from_mins(1))?;

    Ok(path)
}

/// Create a dataset entry and register the `.rrd`s at `paths`, waiting for registration to finish.
///
/// Returns the segment ids in the same order as `paths`.
async fn register_rrds(
    connection: &ConnectionHandle,
    dataset_name: &str,
    dataset_id_str: &str,
    paths: &[&std::path::Path],
) -> Result<Vec<SegmentId>, Box<dyn Error>> {
    let dataset_id = re_tuid::Tuid::from_str(dataset_id_str).expect("Failed to parse TUID");

    let entry = connection
        .client()
        .await?
        .create_dataset_entry(EntryName::new(dataset_name)?, Some(dataset_id.into()))
        .await?;

    let mut data_sources = Vec::with_capacity(paths.len());
    for path in paths {
        data_sources.push(DataSource::new_rrd(format!(
            "file://{}",
            path.to_str()
                .ok_or_else(|| "Failed to convert path to str".to_owned())?
        ))?);
    }

    let registration = connection
        .register_with_dataset(entry.details.id, data_sources, IfDuplicateBehavior::Error)
        .await?;
    Ok(registration.wait(Duration::from_secs(10)).await?)
}

/// Log a static-only recording and register it with `asset_dataset`, returning its segment id.
///
/// Asset datasets only accept static chunks, so the recording logs its points statically.
pub async fn register_asset(
    connection: &ConnectionHandle,
    asset_dataset: EntryId,
    recording_id: &str,
) -> Result<SegmentId, Box<dyn Error>> {
    let path = recording_rrd(recording_id, |stream| {
        stream
            .log_static(
                "asset_entity",
                &archetypes::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
            )
            .expect("Failed to log static points 3D");
    })?;

    let data_source = DataSource::new_rrd(format!(
        "file://{}",
        path.path()
            .to_str()
            .ok_or_else(|| "Failed to convert path to str".to_owned())?
    ))?;

    let registration = connection
        .register_with_dataset(asset_dataset, vec![data_source], IfDuplicateBehavior::Error)
        .await?;
    registration
        .wait(Duration::from_secs(10))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "Asset registration returned no segment".into())
}

/// Register a `.rbl` blueprint file with `table`'s implicit blueprint dataset and set it as the
/// table's default blueprint, mirroring `TableEntry.register_blueprint` in the Python SDK.
///
/// The viewer fetches this registered blueprint when the table entry is opened, which is what
/// turns the preview column into inline 3D previews.
pub async fn register_table_blueprint(
    connection: &ConnectionHandle,
    table: &TableEntry,
    blueprint_rbl: &std::path::Path,
) -> Result<SegmentId, Box<dyn Error>> {
    let blueprint_dataset = table
        .table_details
        .blueprint_dataset
        .ok_or("table is missing its implicit blueprint dataset")?;

    let data_source = DataSource::new_rrd(format!(
        "file://{}",
        blueprint_rbl
            .to_str()
            .ok_or_else(|| "Failed to convert blueprint path to str".to_owned())?
    ))?;

    let registration = connection
        .register_with_dataset(
            blueprint_dataset,
            vec![data_source],
            IfDuplicateBehavior::Overwrite,
        )
        .await?;
    let segment_id = registration
        .wait(Duration::from_secs(10))
        .await?
        .into_iter()
        .next()
        .ok_or("Blueprint registration returned no segment")?;

    connection
        .client()
        .await?
        .update_table_entry(
            table.details.id,
            TableDetails {
                blueprint_dataset: Some(blueprint_dataset),
                default_blueprint_segment: Some(segment_id.clone()),
            },
        )
        .await?;

    Ok(segment_id)
}
