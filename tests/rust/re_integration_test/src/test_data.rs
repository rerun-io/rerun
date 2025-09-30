use re_protos::cloud::v1alpha1::ext::DataSource;
use re_protos::cloud::v1alpha1::{EntryFilter, EntryKind};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::ConnectionClient;
use re_sdk::time::TimeType;
use re_sdk::{RecordingStreamBuilder, TimeCell};
use std::{error::Error, time::Duration};

pub async fn load_test_data(mut client: ConnectionClient) -> Result<(), Box<dyn Error>> {
    let path = {
        let path = tempfile::NamedTempFile::new()?;
        let stream = RecordingStreamBuilder::new("rerun_example_integration_test")
            .recording_id("new_recording_id")
            .save(path.path())?;

        for x in 0..20 {
            stream.set_time("test_time", TimeCell::new(TimeType::Sequence, x));
        }

        stream.flush_with_timeout(Duration::from_secs(60))?;

        path
    };

    let entries_table = client.find_entries(EntryFilter::default()).await?;
    assert_eq!(entries_table.len(), 1);
    assert_eq!(entries_table[0].name, "__entries");
    assert_eq!(entries_table[0].kind, EntryKind::Table);

    let dataset_name = "my_dataset";

    let entry = client
        .create_dataset_entry(dataset_name.to_owned(), None)
        .await?;

    client
        .register_with_dataset(
            entry.details.id,
            vec![DataSource::new_rrd(format!(
                "file://{}",
                path.path()
                    .to_str()
                    .ok_or_else(|| "Failed to convert path to str".to_owned())?
            ))?],
            IfDuplicateBehavior::Error,
        )
        .await?;

    Ok(())
}
