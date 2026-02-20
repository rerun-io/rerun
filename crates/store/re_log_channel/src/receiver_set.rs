use std::sync::Arc;

use parking_lot::Mutex;

use crate::{LogReceiver, LogSource, SmartMessage};

#[cfg(not(target_arch = "wasm32"))]
use crate::RecvError;

/// A set of connected [`LogReceiver`]s.
///
/// Any receiver that gets disconnected is automatically removed from the set.
#[derive(Default)]
pub struct LogReceiverSet {
    receivers: Mutex<Vec<LogReceiver>>,
}

impl LogReceiverSet {
    pub fn new(receivers: Vec<LogReceiver>) -> Self {
        Self {
            receivers: Mutex::new(receivers),
        }
    }

    pub fn add(&self, r: LogReceiver) {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.push(r);
    }

    /// Are we currently receiving this source?
    pub fn contains(&self, source: &LogSource) -> bool {
        self.receivers
            .lock()
            .iter()
            .any(|src| src.source().is_same_ignoring_uri_fragments(source))
    }

    /// Disconnect from any channel with the given source.
    pub fn remove(&self, source: &LogSource) {
        self.receivers.lock().retain(|r| r.source() != source);
    }

    pub fn retain(&self, mut f: impl FnMut(&LogReceiver) -> bool) {
        self.receivers.lock().retain(|r| f(r));
    }

    pub fn for_each(&self, mut f: impl FnMut(&LogReceiver)) {
        for r in self.receivers.lock().iter() {
            f(r);
        }
    }

    /// Remove all receivers.
    pub fn clear(&self) {
        self.receivers.lock().clear();
    }

    /// Disconnect from any channel with a source pointing at this `uri`.
    pub fn remove_by_uri(&self, needle: &str) {
        self.receivers.lock().retain(|r| match r.source() {
            // retain only sources which:
            // - aren't network sources
            // - don't point at the given `needle`
            LogSource::HttpStream { url, .. } => url != needle,
            LogSource::MessageProxy(url) => url.to_string() != needle,
            LogSource::RedapGrpcStream { uri, .. } => uri.to_string() != needle,

            LogSource::File { .. }
            | LogSource::Stdin
            | LogSource::Sdk
            | LogSource::RrdWebEvent
            | LogSource::JsChannel { .. } => true,
        });
    }

    /// List of connected receiver sources.
    ///
    /// This gets culled after calling one of the `recv` methods.
    pub fn sources(&self) -> Vec<Arc<LogSource>> {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.retain(|r| r.is_connected());
        rx.iter().map(|r| r.source_arc()).collect()
    }

    /// Any connected receivers?
    ///
    /// This gets updated after calling one of the `recv` methods.
    pub fn is_connected(&self) -> bool {
        !self.is_empty()
    }

    /// No connected receivers?
    ///
    /// This gets updated after calling one of the `recv` methods.
    pub fn is_empty(&self) -> bool {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.retain(|r| r.is_connected());
        rx.is_empty()
    }

    /// Sum queue length of all receivers.
    pub fn queue_len(&self) -> usize {
        re_tracing::profile_function!();
        let rx = self.receivers.lock();
        rx.iter().map(|r| r.len()).sum()
    }

    /// Blocks until a message is ready to be received,
    /// or we are empty.
    #[cfg(not(target_arch = "wasm32"))] // Cannot block on web
    pub fn recv(&self) -> Result<SmartMessage, RecvError> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        if rx.is_empty() {
            // Have to early out here, because `Select::select` will panic if there are no channels to select from.
            return Err(RecvError);
        }

        let mut sel = re_quota_channel::Select::new();
        for r in rx.iter() {
            sel.recv(r.inner());
        }

        let oper = sel.select();
        let index = oper.index();
        oper.recv(rx[index].inner()).map_err(|_err| RecvError)
    }

    /// Returns immediately if there is nothing to receive.
    pub fn try_recv(&self) -> Option<(Arc<LogSource>, SmartMessage)> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        if rx.is_empty() {
            return None;
        }

        let mut sel = re_quota_channel::Select::new();
        for r in rx.iter() {
            sel.recv(r.inner());
        }

        let oper = sel.try_select().ok()?;
        let index = oper.index();
        if let Ok(msg) = oper.recv(rx[index].inner()) {
            return Some((rx[index].source_arc(), msg));
        }

        // Nothing ready to receive, but we must poll all receivers to update their `connected` status.
        // Why use `select` first? Because `select` is fair (random) when there is contention.
        for rx in rx.iter() {
            if let Ok(msg) = rx.try_recv() {
                return Some((rx.source_arc(), msg));
            }
        }

        None
    }

    #[cfg(not(target_arch = "wasm32"))] // Cannot block on web
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Option<(Arc<LogSource>, SmartMessage)> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        if rx.is_empty() {
            return None;
        }

        let mut sel = re_quota_channel::Select::new();
        for r in rx.iter() {
            sel.recv(r.inner());
        }

        let oper = sel.select_timeout(timeout).ok()?;
        let index = oper.index();
        if let Ok(msg) = oper.recv(rx[index].inner()) {
            return Some((rx[index].source_arc(), msg));
        }

        // Nothing ready to receive, but we must poll all receivers to update their `connected` status.
        // Why use `select` first? Because `select` is fair (random) when there is contention.
        for rx in rx.iter() {
            if let Ok(msg) = rx.try_recv() {
                return Some((rx.source_arc(), msg));
            }
        }

        None
    }
}

impl re_byte_size::MemUsageTreeCapture for LogReceiverSet {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let mut tree = re_byte_size::MemUsageNode::default();
        self.for_each(|receiver| {
            tree.add(
                receiver.source().to_string(),
                receiver.inner().current_bytes(),
            );
        });
        tree.into_tree()
    }
}

#[test]
fn test_receive_set() {
    use re_log_types::StoreId;

    use crate::{LogSource, log_channel};

    let timeout = std::time::Duration::from_millis(100);

    let (tx_file, rx_file) = log_channel(LogSource::File {
        path: "path".into(),
        follow: false,
    });
    let (tx_sdk, rx_sdk) = log_channel(LogSource::Sdk);

    let set = LogReceiverSet::default();

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(set.sources(), vec![]);

    set.add(rx_file);

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(
        set.sources(),
        vec![Arc::new(LogSource::File {
            path: "path".into(),
            follow: false
        })]
    );

    set.add(rx_sdk);

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(
        set.sources(),
        vec![
            Arc::new(LogSource::File {
                path: "path".into(),
                follow: false
            }),
            Arc::new(LogSource::Sdk)
        ]
    );

    tx_sdk
        .send(crate::DataSourceMessage::UiCommand(
            crate::DataSourceUiCommand::SetUrlFragment {
                store_id: StoreId::empty_recording(),
                fragment: "#foo".into(),
            },
        ))
        .unwrap();
    assert_eq!(set.try_recv().unwrap().0, Arc::new(LogSource::Sdk));

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(set.sources().len(), 2);

    drop(tx_sdk);

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(
        set.sources(),
        vec![Arc::new(LogSource::File {
            path: "path".into(),
            follow: false
        })]
    );

    drop(tx_file);

    assert!(set.try_recv().is_none());
    assert!(set.recv_timeout(timeout).is_none());
    assert_eq!(set.sources(), vec![]);
}
