use std::time::Duration;

use anyhow::Context as _;
use futures::StreamExt as _;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::watch_events_response::Kind;
use re_protos::cloud::v1alpha1::{DeleteEntryRequest, WatchEventsRequest};

use super::common::RerunCloudServiceExt as _;

type Result<T = ()> = anyhow::Result<T>;

/// How long to wait for an expected event before giving up.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn watch_events_entry_created(service: impl RerunCloudService) -> Result {
    // Subscribe *before* triggering: the server only delivers events sent after we subscribe.
    let stream = service
        .watch_events(tonic::Request::new(WatchEventsRequest::default()))
        .await
        .context("failed to watch events")?
        .into_inner();
    let mut stream = std::pin::pin!(stream);

    let dataset_entry = service.create_dataset_entry_with_name("my_dataset").await;

    let event = tokio::time::timeout(EVENT_TIMEOUT, stream.next())
        .await
        .context("timed out waiting for event")?
        .context("event stream ended unexpectedly")?
        .context("event stream returned an error")?;

    assert!(
        matches!(event.kind, Some(Kind::EntryCreated(event)) if event.id == Some(dataset_entry.details.id.into())),
        "unexpected event: {event:?}"
    );

    Ok(())
}

pub async fn watch_events_entry_deleted(service: impl RerunCloudService) -> Result {
    let dataset_entry = service.create_dataset_entry_with_name("my_dataset").await;

    let stream = service
        .watch_events(tonic::Request::new(WatchEventsRequest::default()))
        .await
        .context("failed to watch events")?
        .into_inner();
    let mut stream = std::pin::pin!(stream);

    service
        .delete_entry(tonic::Request::new(DeleteEntryRequest {
            id: Some(dataset_entry.details.id.into()),
        }))
        .await
        .context("failed to delete entry")?;

    let event = tokio::time::timeout(EVENT_TIMEOUT, stream.next())
        .await
        .context("timed out waiting for event")?
        .context("event stream ended unexpectedly")?
        .context("event stream returned an error")?;

    assert!(
        matches!(event.kind, Some(Kind::EntryDeleted(event)) if event.id == Some(dataset_entry.details.id.into())),
        "unexpected event: {event:?}"
    );

    Ok(())
}
