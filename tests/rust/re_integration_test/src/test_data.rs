use std::error::Error;
use std::str::FromStr as _;
use std::time::Duration;

use futures::StreamExt as _;

use re_protos::cloud::v1alpha1::ext as cloud_ext;
use re_protos::cloud::v1alpha1::ext::{
    DataSource, QueryTasksOnCompletionResponse, TableDetails, TableEntry,
};
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionClient;
use re_sdk::external::re_tuid;
use re_sdk::time::TimeType;
use re_sdk::{RecordingStreamBuilder, TimeCell};
use re_sdk_types::SegmentId;
use re_viewer::external::re_sdk_types::{archetypes, components::Color};

pub async fn load_test_data(mut client: ConnectionClient) -> Result<SegmentId, Box<dyn Error>> {
    load_test_data_with_name(
        &mut client,
        "my_dataset",
        "187b552b95a5c2f73f37894708825ba5",
        "new_recording_id",
    )
    .await
}

pub async fn load_test_data_with_name(
    client: &mut ConnectionClient,
    dataset_name: &str,
    dataset_id_str: &str,
    recording_id: &str,
) -> Result<SegmentId, Box<dyn Error>> {
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
        .find_entries(EntryFilter::default().with_entry_kind(EntryKind::Table))
        .await?;
    assert_eq!(entries_table.len(), 1);
    assert_eq!(entries_table[0].name, re_protos::EntryName::entries_table());
    assert_eq!(entries_table[0].kind, EntryKind::Table);

    let segment_ids = register_rrds(client, dataset_name, dataset_id_str, &[path.path()]).await?;
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
    client: &mut ConnectionClient,
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
    register_rrds(client, dataset_name, dataset_id_str, &path_refs).await
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
        .save(path.path())?;

    log_data(&stream);

    stream.flush_with_timeout(Duration::from_secs(60))?;

    Ok(path)
}

/// Create a dataset entry and register the `.rrd`s at `paths`, waiting for registration to finish.
///
/// Returns the segment ids in the same order as `paths`.
async fn register_rrds(
    client: &mut ConnectionClient,
    dataset_name: &str,
    dataset_id_str: &str,
    paths: &[&std::path::Path],
) -> Result<Vec<SegmentId>, Box<dyn Error>> {
    let dataset_id = re_tuid::Tuid::from_str(dataset_id_str).expect("Failed to parse TUID");

    let entry = client
        .create_dataset_entry(dataset_name.to_owned(), Some(dataset_id.into()))
        .await?;

    let mut data_sources = Vec::with_capacity(paths.len());
    for path in paths {
        data_sources.push(DataSource::new_rrd(format!(
            "file://{}",
            path.to_str()
                .ok_or_else(|| "Failed to convert path to str".to_owned())?
        ))?);
    }

    let items = client
        .register_with_dataset(entry.details.id, data_sources, IfDuplicateBehavior::Error)
        .await?
        .1;

    let mut segment_ids = Vec::with_capacity(items.len());
    let mut task_ids = Vec::with_capacity(items.len());
    for item in items {
        let cloud_ext::RegisterWithDatasetTaskDescriptor {
            segment_id,
            segment_type: _,
            storage_url: _,
            task_id,
        } = item;
        segment_ids.push(segment_id);
        task_ids.push(task_id);
    }

    wait_for_tasks(client, task_ids).await?;

    Ok(segment_ids)
}

/// Register a `.rbl` blueprint file with `table`'s implicit blueprint dataset and set it as the
/// table's default blueprint, mirroring `TableEntry.register_blueprint` in the Python SDK.
///
/// The viewer fetches this registered blueprint when the table entry is opened, which is what
/// turns the preview column into inline 3D previews.
pub async fn register_table_blueprint(
    client: &mut ConnectionClient,
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

    let items = client
        .register_with_dataset(
            blueprint_dataset,
            vec![data_source],
            IfDuplicateBehavior::Overwrite,
        )
        .await?
        .1;

    let mut segment_id = None;
    let mut task_ids = Vec::with_capacity(items.len());
    for item in items {
        segment_id = Some(item.segment_id);
        task_ids.push(item.task_id);
    }
    let segment_id = segment_id.ok_or("Blueprint registration returned no segment")?;

    wait_for_tasks(client, task_ids).await?;

    client
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

/// Wait for the given registration tasks to complete, returning an error if any task failed.
async fn wait_for_tasks(
    client: &mut ConnectionClient,
    task_ids: Vec<re_protos::common::v1alpha1::TaskId>,
) -> Result<(), Box<dyn Error>> {
    let timeout = Duration::from_secs(10);
    let mut response_stream = client.query_tasks_on_completion(task_ids, timeout).await?;

    while let Some(response) = response_stream.next().await {
        let response: QueryTasksOnCompletionResponse = response?.try_into()?;
        let batch = response.data;
        let statuses = cloud_ext::QueryTasksDataframe::COLUMN_EXEC_STATUS.extract(&batch)?;
        let msgs = cloud_ext::QueryTasksDataframe::COLUMN_MSGS.extract(&batch)?;

        for (status, msg) in std::iter::zip(&statuses, &msgs) {
            if status != "success" {
                let msg = msg.unwrap_or_default();
                return Err(format!("Registration task failed with status {status}: {msg}").into());
            }
        }
    }

    Ok(())
}
