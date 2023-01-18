use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, Write},
    time::Duration,
};

use crossbeam::{
    channel::{self, RecvError},
    select,
};

use re_log::{error, trace};

use crate::{Config, Event, PostHogSink, SinkError};

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
pub struct Pipeline {
    event_tx: channel::Sender<Result<Event, RecvError>>,
}

// TODO: load and send previous unsent sessions at boot
// TODO: grab session_id from file_name directly I guess
// TODO: delete fully sent sessions

impl Pipeline {
    pub fn new(config: &Config, tick: Duration, sink: PostHogSink) -> Result<Self, PipelineError> {
        let (event_tx, event_rx) = channel::unbounded(); // TODO: bounded?

        // TODO: try to send on shutdown as best as possible

        let data_path = config.data_dir().to_owned();

        // NOTE: We purposefully drop the handle and forget about this thread.
        //
        // Joining this thread is not a viable strategy for two reasons:
        // 1. We _never_ want to delay the shutdown process, analytics must never be in the way.
        // 2. We need to deal with unexpected shutdowns anyway (crashes, SIGINT, SIGKILL), and we
        //    do indeed.
        //
        // The worst thing that can happen is that the user kills the app at the exact moment where
        // the catchup thread has sent the data over to the sink and got a response back, but
        // hasn't deleted the associated files yet.
        // This will result in duplicated data that we can easily deduplicate at query time using
        // the session and event IDs.
        _ = std::thread::Builder::new()
            .name("pipeline_catchup".into())
            .spawn({
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

        // NOTE: We purposefully drop the handle and forget about this thread.
        //
        // Joining this thread is not a viable strategy for two reasons:
        // 1. We _never_ want to delay the shutdown process, analytics must never be in the way.
        // 2. We need to deal with unexpected shutdowns anyway (crashes, SIGINT, SIGKILL), and we
        //    do indeed.
        //
        // TODO: what are the possible situations here?
        _ = std::thread::Builder::new().name("pipeline".into()).spawn({
            let config = config.clone();
            move || {
                let analytics_id = &config.analytics_id;
                let session_id = &config.session_id.to_string();

                trace!(%analytics_id, %session_id, "pipeline thread started");
                let res = realtime_pipeline(&config, &sink, session_file, tick, event_rx);
                trace!(%analytics_id, %session_id, ?res, "pipeline thread shut down");
            }
        });

        Ok(Self { event_tx })
    }

    pub fn record(&self, event: Event) {
        // NOTE: We ignore the error on purpose.
        //
        // The only way this can fail is if the other end of the channel was previously closed,
        // which we _never_ do.
        // Technically, we should call `.unwrap()` here, but analytics _must never_ be the cause
        // of a crash, so let's not take any unnecessary risk and just ignore the error instead.
        _ = self.event_tx.send(Ok(event));
    }
}

// ---

fn send_unsent_events(config: &Config, sink: &PostHogSink) -> anyhow::Result<()> {
    let data_path = config.data_dir();
    let analytics_id = config.analytics_id.clone();
    let current_session_id = config.session_id.to_string();

    // TODO: send everything at once?
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
            if flush_events(&mut session_file, &analytics_id, session_id, sink).is_ok() {
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

/// Appends the `event` to the active `session_file`.
///
/// On retriable errors, the event to retry is returned.
fn append_event(
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    event: Event,
) -> Option<Event> {
    let mut event_str = match serde_json::to_string(&event) {
        Ok(event_str) => event_str,
        Err(err) => {
            error!(%err, %analytics_id, %session_id, "corrupt analytics event: discarding");
            return None;
        }
    };
    event_str.push('\n');

    if let Err(err) = session_file.write_all(event_str.as_bytes()) {
        // TODO: we're not gonna have a good time if the write fails halfway... then again
        // there's really no reason it should, so...
        // If that happens, we _could_ detect it and clear that specific line...
        // TODO: actually, it's fine if we handle corrupt lines at real time.
        error!(%err, %analytics_id, %session_id, "couldn't write to analytics data file");
        return Some(event);
    }

    None
}

/// Sends all events currently buffered in the `session_file` down the `sink`.
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
                match serde_json::from_str::<Event>(&event_str) {
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
