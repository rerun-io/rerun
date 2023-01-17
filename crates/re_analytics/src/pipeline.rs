use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, Write},
    path::{Path, PathBuf},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crossbeam::{
    channel::{self, RecvError},
    select,
};
use reqwest::blocking::Client as HttpClient;
use time::OffsetDateTime;

use re_log::{error, trace};
use uuid::Uuid;

use crate::{Config, Event, Property};

// TODO: let's say this is specifically our _native posthog_ pipeline

// TODO: web impl (how does one POST when in web?)
// TODO: endpoint configuration would ideally not live in code...
// TODO: what do we do on web? do we even send stats when running on web?

// ---

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

// TODO: do we want a singleton? do we just pass it around in ctx? let's just do ctx for now

#[derive(Debug)]
pub struct EventPipeline {
    // TODO: not cloning this everytime
    pub analytics_id: String,
    pub session_id: String,

    event_tx: channel::Sender<Result<Event, RecvError>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl Drop for EventPipeline {
    fn drop(&mut self) {
        // TODO: when shutting down, we want to try and POST everything in a fire-and-forget way,
        // so that we don't have to wait for the next time the user uses the viewer (which might be
        // _never_) in order to send the pending events.
        //
        // Problem is: if we do so, we'll end up with duplicated data when we try to send the same
        // batch of events the next time the viewer is spawned... and I don't think there's anywway
        // to deduplicate on ingestion on PostHog's side... though of course we can always
        // deduplicate at query time with session IDs etc.
        if let Err(err) = self.thread_handle.take().unwrap().join() {
            error!(?err, "failed to join analytics thread handle");
        }
    }
}

// TODO:
// - dump to storage (file on native, localstore on web)

impl EventPipeline {
    pub fn new(config: &Config, tick: Duration) -> Result<Self, PipelineError> {
        let (event_tx, event_rx) = channel::unbounded(); // TODO: bounded?

        // let (file_tx, file_rx) = channel::unbounded(); // TODO: bounded?

        // TODO: try to send on shutdown as best as possible

        let data_path = config.data_path().to_owned();
        // TODO: during boot, push existing files into the pipe
        // TODO: file names are session IDs, which are tuids, which are sorted
        // TODO: anyone can edit these files, is that an issue? considering that our write-only key
        // is public anyway, I don't think it matters too much...

        let session_file_path = data_path.join(format!("{}.json", config.session_id));
        let mut session_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .read(true)
            .open(&session_file_path)?;

        let analytics_id = config.analytics_id.clone();
        let session_id = config.session_id.to_string();
        let is_first_run = config.is_first_run();

        let client = PostHogClient::new()?;

        let ticker_rx = crossbeam::channel::tick(tick);

        // TODO: maybe we do it all in one thread tho?
        // TODO: when do we join this one? do we?
        let thread_handle = std::thread::spawn(move || {
            let mut tick_id = 1u64;

            trace!(tick_id, ?session_file_path, %analytics_id, %session_id, "PostHog native pipeline started");

            'recv_loop: loop {
                select! {
                    recv(ticker_rx) -> elapsed => {
                        if !is_first_run {
                            trace!(tick_id, ?session_file_path, %analytics_id, %session_id, "flushing analytics");
                            flush_events(tick_id, &mut session_file, &analytics_id, &session_id, &client);
                        }
                        tick_id += 1;
                    },
                    recv(event_rx) -> event => {
                        let Ok(event) = event.unwrap() else { break 'recv_loop };
                        trace!(
                            tick_id, ?session_file_path, %analytics_id, %session_id,
                            "appending event to current session file..."
                        );
                        append_event(tick_id, &mut session_file, event);
                    },
                }
            }

            trace!(
                tick_id,
                ?session_file_path,
                "PostHog native pipeline shut down"
            );
        });

        Ok(Self {
            analytics_id: config.analytics_id.clone(),
            session_id: config.session_id.to_string(),
            event_tx,
            thread_handle: Some(thread_handle),
        })
    }

    // TODO: there's gonna be some dedup mess in here

    pub fn record(&self, event: Event) {
        self.event_tx
            .send(Ok(event))
            // TODO: can only fail if we close the channel, which we don't... should we, tho?
            .unwrap();
    }
}

fn append_event(tick_id: u64, session_file: &mut File, event: Event) {
    // TODO: how could this ever fail?
    let mut event_str = serde_json::to_string(&event).unwrap();
    event_str.push('\n');
    if let Err(err) = session_file.write_all(event_str.as_bytes()) {
        // TODO: we're not gonna have a good time if the write fails halfway... then again
        // there's really no reason it should, so...
        error!(%err, "couldn't write to analytics data file");

        // TODO: i guess we truncate it and move on then?
        // TODO: also gl testing this...
    }
}

fn flush_events(
    tick_id: u64,
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    client: &PostHogClient,
) {
    if let Err(err) = session_file.seek(std::io::SeekFrom::Start(0)) {
        // TODO: ???
        error!(%err, "couldn't seek into analytics data file");
    }

    let events = BufReader::new(&*session_file).lines().filter_map(|event_str|
        match event_str {
            Ok(event_str) => {
                match serde_json::from_str::<Event>(&event_str) {
                    Ok(event) => Some(event),
                    Err(err) => {
                        error!(%err, "couldn't deserialize event from analytics data file: dropping event");
                        None
                    },
                }
            }
            Err(err) => {
                error!(%err, "couldn't read line from analytics data file: dropping event");
                None
            }
        }).collect::<Vec<_>>();

    if events.is_empty() {
        trace!(
            tick_id,
            %analytics_id,
            %session_id,
            "cancelling flush: no events"
        );
        return;
    }

    let events = events
        .iter()
        .map(|event| PostHogEvent::from_event(&analytics_id, &session_id, event))
        .collect::<Vec<_>>();
    let batch = PostHogBatch::from_events(&events);

    if let Err(err) = client.send(dbg!(&batch)) {
        error!(%err, "failed to send analytics to PostHog, will try again later");
        return;
    }
    trace!(
        tick_id,
        %analytics_id,
        %session_id,
        nb_events = batch.batch.len(),
        "batch successfully sent to PostHog"
    );

    if let Err(err) = session_file.set_len(0) {
        error!(%err, "couldn't truncate analytics data file");
        // TODO: wat now?
    }
}

// ---

// TODO(cmc): abstract away when comes the day where we want to re-use the pipeline for another
// provider.

// TODO: events other than capture, especially views

// See https://posthog.com/docs/api/post-only-endpoints#single-event.
#[derive(Debug, serde::Serialize)]
struct PostHogEvent<'a> {
    #[serde(with = "::time::serde::rfc3339")]
    timestamp: OffsetDateTime,
    event: &'a str,
    distinct_id: &'a str,
    properties: HashMap<&'a str, serde_json::Value>,
}

impl<'a> PostHogEvent<'a> {
    fn from_event(analytics_id: &'a str, session_id: &'a str, event: &'a Event) -> Self {
        let properties = event
            .props
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str(),
                    match value {
                        &Property::Integer(v) => v.into(),
                        &Property::Float(v) => v.into(),
                        Property::String(v) => v.as_str().into(),
                        &Property::Bool(v) => v.into(),
                    },
                )
            })
            .chain([
                // TODO: surely there has to be some nicer way of dealing with sessions...
                ("session_id", session_id.into()),
            ])
            // TODO: application_id (hashed)
            // TODO: recording_id (hashed)
            // (unless these belong only to viewer-opened?)
            .collect();

        Self {
            timestamp: event.time_utc,
            event: event.name.as_ref(),
            distinct_id: analytics_id,
            properties,
        }
    }
}

// TODO: no idea how we're supposed to deal with some entity actively trashing our analytics?
// only way I can think of is to go through our own server first and have asymetric encryption...

const PUBLIC_POSTHOG_PROJECT_KEY: &str = "phc_XD1QbqTGdPJbzdVCbvbA9zGOG38wJFTl8RAwqMwBvTY";

// See https://posthog.com/docs/api/post-only-endpoints#batch-events.
#[derive(Debug, serde::Serialize)]
struct PostHogBatch<'a> {
    api_key: &'static str,
    batch: &'a [PostHogEvent<'a>],
}

impl<'a> PostHogBatch<'a> {
    fn from_events(events: &'a [PostHogEvent<'a>]) -> Self {
        Self {
            api_key: PUBLIC_POSTHOG_PROJECT_KEY,
            batch: events,
        }
    }
}

struct PostHogClient {
    client: HttpClient,
}

impl PostHogClient {
    fn new() -> Result<Self, PipelineError> {
        use reqwest::header;
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let client = HttpClient::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(120))
            .default_headers(headers)
            .build()?;

        Ok(Self { client })
    }

    // TODO: blocking!
    fn send(&self, batch: &PostHogBatch<'_>) -> Result<(), PipelineError> {
        const URL: &str = "https://eu.posthog.com/capture";

        let resp = self
            .client
            .post(URL)
            .body(serde_json::to_vec(&batch)?)
            .send()?;

        resp.error_for_status().map(|_| ()).map_err(Into::into)
    }
}
