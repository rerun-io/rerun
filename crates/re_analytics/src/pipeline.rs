use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, Write},
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::{
    channel::{self, RecvError},
    select,
};
use reqwest::blocking::Client as HttpClient;
use time::OffsetDateTime;

use re_log::{error, trace};

use crate::{Config, Event, PostHogSink, Property};

// TODO: in general, deal with broken anlytics files (whether it's the file as a whole or just some
// lines in there).

// TODO: let's say this is specifically our _native posthog_ pipeline

// TODO: endpoint configuration would ideally not live in code...

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

impl EventPipeline {
    pub fn new(config: &Config, tick: Duration, sink: PostHogSink) -> Result<Self, PipelineError> {
        let (event_tx, event_rx) = channel::unbounded(); // TODO: bounded?

        // TODO: try to send on shutdown as best as possible

        let data_path = config.data_dir().to_owned();
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

        let ticker_rx = crossbeam::channel::tick(tick);

        // TODO: when do we join this one? do we?
        // TODO: name the thread too
        let thread_handle = std::thread::spawn(move || {
            let mut tick_id = 1u64;

            trace!(tick_id, ?session_file_path, %analytics_id, %session_id, "PostHog native pipeline started");

            'recv_loop: loop {
                select! {
                    recv(ticker_rx) -> _elapsed => {
                        if !is_first_run {
                            trace!(tick_id, ?session_file_path, %analytics_id, %session_id, "flushing analytics");
                            flush_events(tick_id, &mut session_file, &analytics_id, &session_id, &sink);
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

fn append_event(_tick_id: u64, session_file: &mut File, event: Event) {
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
    sink: &PostHogSink,
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
                        // TODO: if we're here, we gotta drop the original file or something...
                        // TODO: also this probably shouldn't be an error!()...
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
        return;
    }

    if let Err(err) = sink.send(analytics_id, session_id, &events) {
        error!(%err, "failed to send analytics to PostHog, will try again later");
        return;
    }
    trace!(
        tick_id,
        %analytics_id,
        %session_id,
        nb_events = events.len(),
        "batch successfully sent to PostHog"
    );

    if let Err(err) = session_file.set_len(0) {
        error!(%err, "couldn't truncate analytics data file");
        // TODO: wat now?
    }
}
