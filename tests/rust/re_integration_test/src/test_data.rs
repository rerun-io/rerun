use re_grpc_client::ConnectionClient;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_protos::frontend::v1alpha1::EntryFilter;
use re_protos::manifest_registry::v1alpha1::ext::DataSource;
use re_sdk::time::TimeType;
use re_sdk::{RecordingStreamBuilder, TimeCell};
use std::error::Error;

pub async fn load_test_data(mut client: ConnectionClient) -> Result<(), Box<dyn Error>> {
    let path = {
        let path = tempfile::NamedTempFile::new()?;
        let stream = RecordingStreamBuilder::new("rerun_example_integration_test")
            .recording_id("new_recording_id")
            .save(path.path())?;

        for x in 0..20 {
            stream.set_time("test_time", TimeCell::new(TimeType::Sequence, x));
        }

        stream.flush_blocking();

        path
    };

    assert!(
        client
            .find_entries(EntryFilter::default())
            .await?
            .is_empty()
    );

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
