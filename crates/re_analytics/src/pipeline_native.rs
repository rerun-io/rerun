use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, Write},
    time::Duration,
};

use crossbeam::{
    channel::{self, RecvError},
    select,
};

use re_log::{error, trace, warn, warn_once};

use crate::{Config, Event, PostHogSink, SinkError};

// TODO(cmc): abstract away the concept of a `Pipeline` behind an actual trait when comes the time
// to support more than just PostHog.

// ---

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// An eventual, at-least-once(-ish) event pipeline, backed by a write-ahead log on the local disk.
///
/// Flushing of the WAL is entirely left up to the OS page cache, hance the -ish.
#[derive(Debug)]
pub struct Pipeline {
    event_tx: channel::Sender<Result<Event, RecvError>>,
}

impl Pipeline {
    pub fn new(
        config: &Config,
        tick: Duration,
        sink: PostHogSink,
    ) -> Result<Option<Self>, PipelineError> {
        if !config.analytics_enabled {
            return Ok(None);
        }

        let (event_tx, event_rx) = channel::bounded(2048);

        let data_path = config.data_dir().to_owned();

        let session_file_path = data_path.join(format!("{}.json", config.session_id));
        let session_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .read(true)
            .open(session_file_path)?;

        // NOTE: We purposefully drop the handles and just forget about all pipeline threads.
        //
        // Joining these threads is not a viable strategy for two reasons:
        // 1. We _never_ want to delay the shutdown process, analytics must never be in the way.
        // 2. We need to deal with unexpected shutdowns anyway (crashes, SIGINT, SIGKILL, ...),
        //    and we do indeed.
        //
        // This is an at-least-once pipeline: in the worst case, unexpected shutdowns will lead to
        // _eventually_ duplicated data.
        //
        // The duplication part comes from the fact that we might successfully flush events down
        // the sink but still fail to remove and/or truncate the file.
        // The eventual part comes from the fact that this only runs as part of the Rerun viewer,
        // and as such there's no guarantee it will ever run again, even if there's pending data.

        _ = std::thread::Builder::new()
            .name("pipeline_catchup".into())
            .spawn({
                let config = config.clone();
                let sink = sink.clone();
                move || {
                    let analytics_id = &config.analytics_id;
                    let session_id = &config.session_id.to_string();

                    trace!(%analytics_id, %session_id, "pipeline catchup thread started");
                    let res = flush_pending_events(&config, &sink);
                    trace!(%analytics_id, %session_id, ?res, "pipeline catchup thread shut down");
                }
            });

        _ = std::thread::Builder::new().name("pipeline".into()).spawn({
            let config = config.clone();
            let event_tx = event_tx.clone();
            move || {
                let analytics_id = &config.analytics_id;
                let session_id = &config.session_id.to_string();

                trace!(%analytics_id, %session_id, "pipeline thread started");
                let res =
                    realtime_pipeline(&config, &sink, session_file, tick, &event_tx, &event_rx);
                trace!(%analytics_id, %session_id, ?res, "pipeline thread shut down");
            }
        });

        Ok(Some(Self { event_tx }))
    }

    pub fn record(&self, event: Event) {
        try_send_event(&self.event_tx, event);
    }
}

// ---

fn try_send_event(event_tx: &channel::Sender<Result<Event, RecvError>>, event: Event) {
    match event_tx.try_send(Ok(event)) {
        Ok(_) => {}
        Err(channel::TrySendError::Full(_)) => {
            trace!("dropped event, analytics channel is full");
        }
        Err(channel::TrySendError::Disconnected(_)) => {
            // The only way this can happen is if the other end of the channel was previously
            // closed, which we _never_ do.
            // Technically, we should call `.unwrap()` here, but analytics _must never_ be the
            // cause of a crash, so let's not take any unnecessary risk and just ignore the
            // error instead.
            warn_once!("dropped event, analytics channel is disconnected");
        }
    }
}

fn flush_pending_events(config: &Config, sink: &PostHogSink) -> anyhow::Result<()> {
    let data_path = config.data_dir();
    let analytics_id = config.analytics_id.clone();
    let current_session_id = config.session_id.to_string();

    let read_dir = data_path.read_dir()?;
    for entry in read_dir {
        // NOTE: all of these can only be transient I/O errors, so no reason to delete the
        // associated file; we'll retry later.
        let Ok(entry) = entry else { continue; };
        let Ok(name) = entry.file_name().into_string() else { continue; };
        let Ok(metadata) = entry.metadata() else { continue; };
        let path = entry.path();

        if metadata.is_file() {
            let Some(session_id) = name.strip_suffix(".json") else { continue; };

            if session_id == current_session_id {
                continue;
            }

            let Ok(mut session_file) = File::open(&path) else { continue; };
            match flush_events(&mut session_file, &analytics_id, session_id, sink) {
                Ok(_) => {
                    trace!(%analytics_id, %session_id, ?path, "flushed pending events");
                    match std::fs::remove_file(&path) {
                        Ok(_) => trace!(%analytics_id, %session_id, ?path, "removed session file"),
                        Err(err) => {
                            // NOTE: this will eventually lead to duplicated data, though we'll be
                            // able to deduplicate it at query time.
                            trace!(%analytics_id, %session_id, ?path, %err,
                                "failed to remove session file");
                        }
                    }
                }
                Err(err) => trace!(%analytics_id, %session_id, ?path, %err,
                    "failed to flush pending events"),
            }
        }
    }

    Ok::<_, anyhow::Error>(())
}

#[allow(clippy::unnecessary_wraps)]
fn realtime_pipeline(
    config: &Config,
    sink: &PostHogSink,
    mut session_file: File,
    tick: Duration,
    event_tx: &channel::Sender<Result<Event, RecvError>>,
    event_rx: &channel::Receiver<Result<Event, RecvError>>,
) -> anyhow::Result<()> {
    let analytics_id = config.analytics_id.clone();
    let session_id = config.session_id.to_string();
    let is_first_run = config.is_first_run();

    let ticker_rx = crossbeam::channel::tick(tick);

    let on_tick = |session_file: &mut _, _elapsed| {
        if is_first_run {
            // We never send data on first run, to give end users an opportunity to opt-out.
            return;
        }

        if let Err(err) = flush_events(session_file, &analytics_id, &session_id, sink) {
            warn!(%err, %analytics_id, %session_id, "couldn't flush analytics data file");
            // We couldn't flush the session file: keep it intact so that we can retry later.
            return;
        }

        if let Err(err) = session_file.set_len(0) {
            warn!(%err, %analytics_id, %session_id, "couldn't truncate analytics data file");
            // We couldn't truncate the session file: we'll have to keep it intact for now, which
            // will result in duplicated data that we'll be able to deduplicate at query time.
            return;
        }
        if let Err(err) = session_file.rewind() {
            // We couldn't reset the session file... That one is a bit messy and will likely break
            // analytics for the entire duration of this session, but that really _really_ should
            // never happen.
            warn!(%err, %analytics_id, %session_id, "couldn't seek into analytics data file");
        }
    };

    let on_event = |session_file: &mut _, event| {
        trace!(
            %analytics_id, %session_id,
            "appending event to current session file..."
        );
        if let Err(event) = append_event(session_file, &analytics_id, &session_id, event) {
            // We failed to append the event to the current session, so push it back at the end of
            // the queue to be retried later on.
            try_send_event(event_tx, event);
        }
    };

    loop {
        select! {
            recv(ticker_rx) -> elapsed => on_tick(&mut session_file, elapsed),
            recv(event_rx) -> event => {
                let Ok(event) = event.unwrap() else { break; };
                on_event(&mut session_file, event);
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
) -> Result<(), Event> {
    let mut event_str = match serde_json::to_string(&event) {
        Ok(event_str) => event_str,
        Err(err) => {
            error!(%err, %analytics_id, %session_id, "corrupt analytics event: discarding");
            return Ok(());
        }
    };
    event_str.push('\n');

    // NOTE: We leave the how and when to flush the file entirely up to the OS page cache, kinda
    // breaking our promise of at-least-once semantics, though this is more than enough
    // considering the use case at hand.
    if let Err(err) = session_file.write_all(event_str.as_bytes()) {
        // NOTE: If the write failed halfway through for some crazy reason, we'll end up with a
        // corrupt row in the analytics file, that we'll simply discard later on.
        // We'll try to write a linefeed one more time, just in case, to avoid potentially
        // impacting other events.
        _ = session_file.write_all(b"\n");
        warn!(%err, %analytics_id, %session_id, "couldn't write to analytics data file");
        return Err(event);
    }

    Ok(())
}

/// Sends all events currently buffered in the `session_file` down the `sink`.
fn flush_events(
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    sink: &PostHogSink,
) -> Result<(), SinkError> {
    if let Err(err) = session_file.rewind() {
        warn!(%err, %analytics_id, %session_id, "couldn't seek into analytics data file");
        return Err(err.into());
    }

    let events = BufReader::new(&*session_file)
        .lines()
        .filter_map(|event_str| match event_str {
            Ok(event_str) => {
                match serde_json::from_str::<Event>(&event_str) {
                    Ok(event) => Some(event),
                    Err(err) => {
                        // NOTE: This is effectively where we detect posssible half-writes.
                        error!(%err, %analytics_id, %session_id,
                            "couldn't deserialize event from analytics data file: dropping it");
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
        warn!(%err, "failed to send analytics down the sink, will try again later");
        return Err(err);
    }

    trace!(
        %analytics_id,
        %session_id,
        num_events = events.len(),
        "events successfully flushed"
    );

    Ok(())
}
