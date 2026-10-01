use std::fs::{File, OpenOptions, TryLockError};
use std::io::{BufRead as _, BufReader, Seek as _, Write as _};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crossbeam::{channel, select};

use super::AbortSignal;
use super::sink::PostHogSink;
use crate::{AnalyticsEvent, Config, FlushError};

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
        if re_log::env_var_is_truthy(crate::ENV_FORCE_ANALYTICS) {
            re_log::debug_once!("Analytics enabled by environment variable");
        } else if !config.analytics_enabled {
            re_log::debug_once!("Analytics disabled by configuration");
            return Ok(None);
        }

        let sink = PostHogSink::default();
        let (event_tx, event_rx) = channel::bounded(2048);
        let (flush_done_tx, flush_done_rx) = channel::bounded(1);
        let abort_signal = AbortSignal::new();

        let data_path = config.data_dir().to_owned();

        std::fs::create_dir_all(data_path.clone())?;

        let session_file_path = data_path.join(format!("{}.json", config.session_id));
        let session_file = create_locked_session_file(&session_file_path)?;

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
        Ok(()) => {}
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

            let mut session_file = match open_pending_file(&path) {
                Ok(Some(session_file)) => session_file,
                Ok(None) => {
                    re_log::trace!(%analytics_id, %session_id, ?path, "session file still in use");
                    continue;
                }
                Err(err) => {
                    re_log::debug!(%analytics_id, %session_id, ?path, %err,
                        "failed to open session file");
                    continue;
                }
            };
            let session_id: Arc<str> = session_id.into();
            match flush_pending_file(&mut session_file, &path, |session_file| {
                flush_events(session_file, &analytics_id, &session_id, sink, abort_signal)
            }) {
                Ok(()) => {
                    re_log::trace!(%analytics_id, %session_id, ?path, "flushed pending events");
                }
                Err(err) => re_log::trace!(%analytics_id, %session_id, ?path, %err,
                    "failed to flush pending events"),
            }
        }
    }

    Ok(())
}

/// Locked before it gets its discoverable `.json` name, until the returned handle is dropped.
fn create_locked_session_file(path: &Path) -> std::io::Result<File> {
    let tmp_path = path.with_extension("json.tmp");
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .read(true)
        .open(&tmp_path)?;
    if let Err(err) = file.try_lock() {
        std::fs::remove_file(&tmp_path).ok();
        return Err(err.into());
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(file)
}

/// Returns `None` if another process holds the lock.
fn open_pending_file(path: &Path) -> std::io::Result<Option<File>> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(err)) => Err(err),
    }
}

/// Empties the file before deleting it, so a process that already opened it sends nothing.
fn flush_pending_file(
    session_file: &mut File,
    path: &Path,
    flush: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    flush(session_file)?;

    if let Err(err) = session_file.set_len(0) {
        re_log::trace!(?path, %err, "failed to truncate session file");
    }
    if let Err(err) = std::fs::remove_file(path) {
        re_log::trace!(?path, %err, "failed to remove session file");
    }

    Ok(())
}

#[expect(clippy::needless_return)]
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
            recv(ticker_rx) -> _ => on_flush(&mut session_file),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_session_file(path: &Path, content: &str) {
        std::fs::write(path, content).unwrap();
    }

    fn read_all(file: &mut File) -> String {
        file.rewind().unwrap();
        std::io::read_to_string(file).unwrap()
    }

    #[test]
    fn session_file_locked_by_owner_is_not_flushed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live.json");

        let mut owner = create_locked_session_file(&path).unwrap();
        owner.write_all(b"event\n").unwrap();
        assert!(!path.with_extension("json.tmp").exists());

        assert!(open_pending_file(&path).unwrap().is_none());
        assert!(path.exists());

        drop(owner);

        let mut session_file = open_pending_file(&path).unwrap().unwrap();
        let mut sent = Vec::new();
        flush_pending_file(&mut session_file, &path, |file| {
            sent.push(read_all(file));
            Ok(())
        })
        .unwrap();

        assert_eq!(sent, vec!["event\n".to_owned()]);
        assert!(!path.exists());
    }

    #[test]
    fn session_file_is_flushed_at_most_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orphan.json");
        write_session_file(&path, "event\n");

        let mut first = open_pending_file(&path).unwrap().unwrap();
        let mut second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        assert!(matches!(second.try_lock(), Err(TryLockError::WouldBlock)));

        let mut sent = Vec::new();
        flush_pending_file(&mut first, &path, |file| {
            sent.push(read_all(file));
            Ok(())
        })
        .unwrap();
        drop(first);

        second.try_lock().unwrap();
        assert_eq!(read_all(&mut second), "");
        assert_eq!(sent, vec!["event\n".to_owned()]);
        assert!(open_pending_file(&path).is_err());
    }

    #[test]
    fn failed_flush_keeps_session_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orphan.json");
        write_session_file(&path, "event\n");

        let mut session_file = open_pending_file(&path).unwrap().unwrap();
        flush_pending_file(&mut session_file, &path, |_| {
            Err(std::io::Error::other("network down"))
        })
        .unwrap_err();
        drop(session_file);

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "event\n");
    }
}
