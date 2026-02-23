use std::fs::{File, OpenOptions};
use std::io::{BufRead as _, BufReader, Seek as _, Write as _};
use std::sync::Arc;
use std::time::Duration;

use crossbeam::{channel, select};

use super::AbortSignal;
use super::sink::PostHogSink;
use crate::{AnalyticsEvent, Config, FlushError};

// This is the environment variable that controls analytics collection.
const ENV_FORCE_ANALYTICS: &str = "FORCE_RERUN_ANALYTICS";

pub enum PipelineEvent {
    Analytics(AnalyticsEvent),
    Flush,
}

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

/// An eventual, at-least-once(-ish) event pipeline, backed by a write-ahead log on the local disk.
///
/// Flushing of the WAL is entirely left up to the OS page cache, hance the -ish.
#[derive(Debug)]
pub struct Pipeline {
    event_tx: channel::Sender<PipelineEvent>,
    flush_done_rx: channel::Receiver<()>,
}

impl Pipeline {
    pub(crate) fn new(config: &Config, tick: Duration) -> Result<Option<Self>, PipelineError> {
        if re_log::env_var_is_truthy(ENV_FORCE_ANALYTICS) {
            re_log::debug_once!("Analytics enabled by environment variable");
        } else {
            if !config.analytics_enabled {
                re_log::debug_once!("Analytics disabled by configuration");
                return Ok(None);
            }
            if std::env::var("CI").is_ok() {
                re_log::debug_once!("Analytics disabled on CI");
                return Ok(None);
            }
            if cfg!(feature = "testing") {
                re_log::debug_once!("Analytics disabled in tests");
                return Ok(None);
            }
            if cfg!(debug_assertions) {
                re_log::debug_once!("Analytics disabled in debug builds");
                return Ok(None);
            }
        }

        let sink = PostHogSink::default();
        let (event_tx, event_rx) = channel::bounded(2048);
        let (flush_done_tx, flush_done_rx) = channel::bounded(1);
        let abort_signal = AbortSignal::new();

        let data_path = config.data_dir().to_owned();

        std::fs::create_dir_all(data_path.clone())?;

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
        // 2. We need to deal with unexpected shutdowns anyway (crashes, SIGINT, SIGKILL, …),
        //    and we do indeed.
        //
        // This is an at-least-once pipeline: in the worst case, unexpected shutdowns will lead to
        // _eventually_ duplicated data.
        //
        // The duplication part comes from the fact that we might successfully flush events down
        // the sink but still fail to remove and/or truncate the file.
        // The eventual part comes from the fact that this only runs as part of the Rerun viewer,
        // and as such there's no guarantee it will ever run again, even if there's pending data.

        if let Err(err) = std::thread::Builder::new()
            .name("pipeline_catchup".into())
            .spawn({
                let config = config.clone();
                let sink = sink.clone();
                let abort_signal = abort_signal.clone();
                move || {
                    let analytics_id = &config.analytics_id;
                    let session_id = &config.session_id.to_string();

                    re_log::trace!(%analytics_id, %session_id, "pipeline catchup thread started");
                    let res = flush_pending_events(&config, &sink, &abort_signal);
                    re_log::trace!(%analytics_id, %session_id, ?res, "pipeline catchup thread shut down");
                }
            })
        {
            re_log::debug!("Failed to spawn analytics thread: {err}");
        }

        if let Err(err) = std::thread::Builder::new().name("pipeline".into()).spawn({
            let config = config.clone();
            let event_tx = event_tx.clone();
            let abort_signal = abort_signal.clone();
            move || {
                let analytics_id = &config.analytics_id;
                let session_id = &config.session_id.to_string();

                re_log::trace!(%analytics_id, %session_id, "pipeline thread started");
                realtime_pipeline(
                    &config,
                    &sink,
                    session_file,
                    tick,
                    &event_tx,
                    &event_rx,
                    &flush_done_tx,
                    &abort_signal,
                );
                re_log::trace!(%analytics_id, %session_id, "pipeline thread shut down");
            }
        }) {
            re_log::debug!("Failed to spawn analytics thread: {err}");
        }

        Ok(Some(Self {
            event_tx,
            flush_done_rx,
        }))
    }

    pub fn record(&self, event: AnalyticsEvent) {
        try_send_event(&self.event_tx, PipelineEvent::Analytics(event));
    }

    /// Tries to flush all pending events to the sink.
    pub fn flush_blocking(&self, timeout: Duration) -> Result<(), FlushError> {
        use crossbeam::channel::RecvTimeoutError;

        re_log::trace!("Flushing analytics events…");
        try_send_event(&self.event_tx, PipelineEvent::Flush);

        self.flush_done_rx
            .recv_timeout(timeout)
            .map_err(|err| match err {
                RecvTimeoutError::Timeout => FlushError::Timeout,
                RecvTimeoutError::Disconnected => FlushError::Closed,
            })
    }
}

// ---

fn try_send_event(event_tx: &channel::Sender<PipelineEvent>, event: PipelineEvent) {
    match event_tx.try_send(event) {
        Ok(_) => {}
        Err(channel::TrySendError::Full(_)) => {
            re_log::trace!("dropped event, analytics channel is full");
        }
        Err(channel::TrySendError::Disconnected(_)) => {
            // The only way this can happen is if the other end of the channel was previously
            // closed, which we _never_ do.
            // Technically, we should call `.unwrap()` here, but analytics _must never_ be the
            // cause of a crash, so let's not take any unnecessary risk and just ignore the
            // error instead.
            re_log::debug_once!("dropped event, analytics channel is disconnected");
        }
    }
}

fn flush_pending_events(
    config: &Config,
    sink: &PostHogSink,
    abort_signal: &AbortSignal,
) -> std::io::Result<()> {
    let data_path = config.data_dir();
    let analytics_id: Arc<str> = config.analytics_id.clone().into();
    let current_session_id = config.session_id.to_string();

    let read_dir = data_path.read_dir()?;
    for entry in read_dir {
        // NOTE: all of these can only be transient I/O errors, so no reason to delete the
        // associated file; we'll retry later.
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let path = entry.path();

        if metadata.is_file() {
            let Some(session_id) = name.strip_suffix(".json") else {
                continue;
            };

            if session_id == current_session_id {
                continue;
            }

            let Ok(mut session_file) = File::open(&path) else {
                continue;
            };
            match flush_events(
                &mut session_file,
                &analytics_id,
                &session_id.into(),
                sink,
                abort_signal,
            ) {
                Ok(_) => {
                    re_log::trace!(%analytics_id, %session_id, ?path, "flushed pending events");
                    match std::fs::remove_file(&path) {
                        Ok(_) => {
                            re_log::trace!(%analytics_id, %session_id, ?path, "removed session file");
                        }
                        Err(err) => {
                            // NOTE: this will eventually lead to duplicated data, though we'll be
                            // able to deduplicate it at query time.
                            re_log::trace!(%analytics_id, %session_id, ?path, %err,
                                "failed to remove session file");
                        }
                    }
                }
                Err(err) => re_log::trace!(%analytics_id, %session_id, ?path, %err,
                    "failed to flush pending events"),
            }
        }
    }

    Ok(())
}

#[expect(clippy::needless_return, clippy::too_many_arguments)]
fn realtime_pipeline(
    config: &Config,
    sink: &PostHogSink,
    mut session_file: File,
    tick: Duration,
    event_tx: &channel::Sender<PipelineEvent>,
    event_rx: &channel::Receiver<PipelineEvent>,
    flush_done_tx: &channel::Sender<()>,
    abort_signal: &AbortSignal,
) {
    let analytics_id: Arc<str> = config.analytics_id.clone().into();
    let session_id: Arc<str> = config.session_id.to_string().into();
    let is_first_run = config.is_first_run();

    let ticker_rx = crossbeam::channel::tick(tick);

    let on_flush = |session_file: &mut _| {
        // A number of things can fail here, in all cases we will stop retrying.
        // The next time the analytics boots up, the catchup thread should handle
        // any remaining events.

        if is_first_run {
            // We never send data on first run, to give end users an opportunity to opt-out.
            return abort_signal.abort();
        }

        if let Err(err) = flush_events(session_file, &analytics_id, &session_id, sink, abort_signal)
        {
            re_log::debug_once!("couldn't flush analytics data file: {err}");
            // We couldn't flush the session file: keep it intact so that we can retry later.
            return abort_signal.abort();
        }

        if let Err(err) = session_file.set_len(0) {
            re_log::debug_once!("couldn't truncate analytics data file: {err}");
            // We couldn't truncate the session file: we'll have to keep it intact for now, which
            // will result in duplicated data that we'll be able to deduplicate at query time.
            return abort_signal.abort();
        }
        if let Err(err) = session_file.rewind() {
            // We couldn't reset the session file… That one is a bit messy and will likely break
            // analytics for the entire duration of this session, but that really _really_ should
            // never happen.
            re_log::debug_once!("couldn't seek into analytics data file: {err}");
            return abort_signal.abort();
        }
    };

    let on_event = |session_file: &mut _, event| {
        re_log::trace!(
            %analytics_id, %session_id,
            "appending event to current session file…"
        );
        if let Err(event) = append_event(session_file, &analytics_id, &session_id, event) {
            // We failed to append the event to the current session, so push it back at the end of
            // the queue to be retried later on.
            try_send_event(event_tx, PipelineEvent::Analytics(event));
        }
    };

    loop {
        select! {
            recv(ticker_rx) -> _elapsed => on_flush(&mut session_file),
            recv(event_rx) -> event => {
                let Ok(event) = event else { break };
                match event {
                    PipelineEvent::Analytics(event) => on_event(&mut session_file, event),
                    PipelineEvent::Flush => {
                        on_flush(&mut session_file);
                        re_quota_channel::send_crossbeam(flush_done_tx, ()).ok();
                    },
                }

            },
        }
        // `on_flush` may have failed and signalled an abort
        // in this case we accept our fate and stop collecting events
        if abort_signal.is_aborted() {
            return;
        }
    }
}

// ---

/// Appends the `event` to the active `session_file`.
///
/// On retriable errors, the event to retry is returned.
fn append_event(
    session_file: &mut File,
    analytics_id: &str,
    session_id: &str,
    event: AnalyticsEvent,
) -> Result<(), AnalyticsEvent> {
    let mut event_str = match serde_json::to_string(&event) {
        Ok(event_str) => event_str,
        Err(err) => {
            re_log::debug!(%err, %analytics_id, %session_id, "corrupt analytics event: discarding");
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
        session_file.write_all(b"\n").ok();
        re_log::debug!(%err, %analytics_id, %session_id, "couldn't write to analytics data file");
        return Err(event);
    }

    Ok(())
}

/// Sends all events currently buffered in the `session_file` down the `sink`.
fn flush_events(
    session_file: &mut File,
    analytics_id: &Arc<str>,
    session_id: &Arc<str>,
    sink: &PostHogSink,
    abort_signal: &AbortSignal,
) -> std::io::Result<()> {
    if let Err(err) = session_file.rewind() {
        re_log::debug!(%err, %analytics_id, %session_id, "couldn't seek into analytics data file");
        return Err(err);
    }

    let events = BufReader::new(&*session_file)
        .lines()
        .filter_map(|event_str| match event_str {
            Ok(event_str) => {
                match serde_json::from_str::<AnalyticsEvent>(&event_str) {
                    Ok(event) => Some(event),
                    Err(err) => {
                        // NOTE: This is effectively where we detect possible half-writes.
                        re_log::debug!(%err, %analytics_id, %session_id,
                            "couldn't deserialize event from analytics data file: dropping it");
                        None
                    }
                }
            }
            Err(err) => {
                re_log::debug!(%err, %analytics_id, %session_id,
                    "couldn't read line from analytics data file: dropping event");
                None
            }
        })
        .collect::<Vec<_>>();

    if events.is_empty() {
        return Ok(());
    }

    sink.send(analytics_id, session_id, &events, abort_signal);

    Ok(())
}
