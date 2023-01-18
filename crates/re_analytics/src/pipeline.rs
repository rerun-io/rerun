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

use re_log::{error, trace};

use crate::{Config, Event, PostHogSink, Property, SinkError};

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

// TODO: just pipeline?
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

// TODO: load and send previous unsent sessions at boot
// TODO: grab session_id from file_name directly I guess
// TODO: delete fully sent sessions

impl EventPipeline {
    pub fn new(config: &Config, tick: Duration, sink: PostHogSink) -> Result<Self, PipelineError> {
        let (event_tx, event_rx) = channel::unbounded(); // TODO: bounded?

        // TODO: try to send on shutdown as best as possible

        let data_path = config.data_dir().to_owned();

        // TODO: named thread
        // TODO: do we care about joining this thread actually?
        let _handle = std::thread::spawn({
            let config = config.clone();
            let sink = sink.clone();
            move || {
                let analytics_id = &config.analytics_id;
                let session_id = &config.session_id.to_string();

                trace!(%analytics_id, %session_id, "pipeline catchup thread started");
                let res = send_unsent_events(&config, &sink);
                trace!(%analytics_id, %session_id, ?res, "pipeline catchup thread shut down");
            }
        });

        let session_file_path = data_path.join(format!("{}.json", config.session_id));
        let session_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .read(true)
            .open(&session_file_path)?;

        // TODO: when do we join this one? do we?
        // TODO: name the thread too
        let thread_handle = std::thread::spawn({
            let config = config.clone();
            move || {
                let analytics_id = &config.analytics_id;
                let session_id = &config.session_id.to_string();

                trace!(%analytics_id, %session_id, "pipeline thread started");
                let res = realtime_pipeline(&config, &sink, session_file, tick, event_rx);
                trace!(%analytics_id, %session_id, ?res, "pipeline thread shut down");
            }
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

// ---

fn send_unsent_events(config: &Config, sink: &PostHogSink) -> anyhow::Result<()> {
    let data_path = config.data_dir();
    let analytics_id = config.analytics_id.clone();
    let current_session_id = config.session_id.to_string();

    let read_dir = data_path.read_dir()?;
    for entry in read_dir {
        // TODO: errors here should definitely _not_ stop the whole loop

        let entry = entry?;

        let path = entry.path();
        let name = entry.file_name().into_string().unwrap(); // TODO

        let is_file = entry.metadata()?.is_file();
        let has_json_suffix = name.ends_with(".json");

        if is_file && has_json_suffix {
            let session_id = name.strip_suffix(".json").unwrap(); // TODO

            if session_id == current_session_id {
                continue;
            }

            let mut session_file = File::open(&path)?;
            if flush_events(&mut session_file, &analytics_id, session_id, &sink).is_ok() {
                std::fs::remove_file(&path)?;
                trace!(%analytics_id, %session_id, ?path, "removed session file");
            }
        }
    }

    Ok::<_, anyhow::Error>(())
}

fn realtime_pipeline(
    config: &Config,
    sink: &PostHogSink,
    mut session_file: File,
    tick: Duration,
    event_rx: channel::Receiver<Result<Event, RecvError>>,
) -> anyhow::Result<()> {
    let analytics_id = config.analytics_id.clone();
    let session_id = config.session_id.to_string();
    let is_first_run = config.is_first_run();

    let ticker_rx = crossbeam::channel::tick(tick);

    loop {
        select! {
            recv(ticker_rx) -> _elapsed => {
                if !is_first_run {
                    flush_events(&mut session_file, &analytics_id, &session_id, &sink);
                    if let Err(err) = session_file.set_len(0) {
                        error!(%err, %analytics_id, %session_id,
                            "couldn't truncate analytics data file");
                        // TODO: wat now?
                    }
                    if let Err(err) = session_file.seek(std::io::SeekFrom::Start(0)) {
                        // TODO: ???
                        error!(%err, %analytics_id, %session_id,
                            "couldn't seek into analytics data file");
                    }
                }
            },
            recv(event_rx) -> event => {
                let Ok(event) = event.unwrap() else { break; };
                trace!(
                    %analytics_id, %session_id,
                    "appending event to current session file..."
                );
                append_event(&mut session_file, &analytics_id, &session_id, event);
            },
        }
    }

    Ok(())
}

// ---

fn append_event(
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    event: Event,
) -> Result<(), PipelineError> {
    let mut event_str = serde_json::to_string(&event)?;
    event_str.push('\n');

    if let Err(err) = session_file.write_all(event_str.as_bytes()) {
        // TODO: we're not gonna have a good time if the write fails halfway... then again
        // there's really no reason it should, so...
        // If that happens, we _could_ detect it and clear that specific line...
        error!(%err, %analytics_id, %session_id, "couldn't write to analytics data file");
        return Err(err.into());
    }

    Ok(())
}

fn flush_events(
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    sink: &PostHogSink,
) -> Result<(), SinkError> {
    if let Err(err) = session_file.seek(std::io::SeekFrom::Start(0)) {
        error!(%err, %analytics_id, %session_id, "couldn't seek into analytics data file");
        return Err(err.into());
    }

    let events = BufReader::new(&*session_file)
        .lines()
        .filter_map(|event_str| match event_str {
            Ok(event_str) => {
                match serde_json::from_str::<Event>(dbg!(&event_str)) {
                    Ok(event) => Some(event),
                    Err(err) => {
                        // TODO: if we're here, we gotta drop the original file or something...
                        // TODO: also this probably shouldn't be an error!()...
                        error!(%err, %analytics_id, %session_id,
                            "couldn't deserialize event from analytics data file: dropping event");
                        None
                    }
                }
            }
            Err(err) => {
                error!(%err, %analytics_id, %session_id,
                    "couldn't read line from analytics data file: dropping event");
                None
            }
        })
        .collect::<Vec<_>>();

    if events.is_empty() {
        return Ok(());
    }

    if let Err(err) = sink.send(analytics_id, session_id, &events) {
        error!(%err, "failed to send analytics to PostHog, will try again later");
        return Err(err);
    }

    trace!(
        %analytics_id,
        %session_id,
        nb_events = events.len(),
        "events successfully flushed"
    );

    Ok(())
}
