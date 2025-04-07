use std::sync::Arc;

use crossbeam::channel::Select;
use parking_lot::Mutex;

use crate::{Receiver, RecvError, SmartChannelSource, SmartMessage};

/// A set of connected [`Receiver`]s.
///
/// Any receiver that gets disconnected is automatically removed from the set.
pub struct ReceiveSet<T: Send> {
    receivers: Mutex<Vec<Receiver<T>>>,
}

impl<T: Send> Default for ReceiveSet<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T: Send> ReceiveSet<T> {
    pub fn new(receivers: Vec<Receiver<T>>) -> Self {
        Self {
            receivers: Mutex::new(receivers),
        }
    }

    pub fn add(&self, r: Receiver<T>) {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.push(r);
    }

    /// Disconnect from any channel with the given source.
    pub fn remove(&self, source: &SmartChannelSource) {
        self.receivers.lock().retain(|r| r.source() != source);
    }

    pub fn retain(&self, mut f: impl FnMut(&Receiver<T>) -> bool) {
        self.receivers.lock().retain(|r| f(r));
    }

    /// Remove all receivers.
    pub fn clear(&self) {
        self.receivers.lock().clear();
    }

    /// Disconnect from any channel with a source pointing at this `uri`.
    #[cfg(target_arch = "wasm32")]
    pub fn remove_by_uri(&self, uri: &str) {
        self.receivers.lock().retain(|r| match r.source() {
            // retain only sources which:
            // - aren't network sources
            // - don't point at the given `uri`
            SmartChannelSource::RrdHttpStream { url, .. } => url != uri,
            SmartChannelSource::MessageProxy(url) => url.to_string() != uri,
            SmartChannelSource::RedapGrpcStream(url) => url.to_string() != uri,

            SmartChannelSource::File(_)
            | SmartChannelSource::Stdin
            | SmartChannelSource::Sdk
            | SmartChannelSource::RrdWebEventListener
            | SmartChannelSource::JsChannel { .. } => true,
        });
    }

    /// List of connected receiver sources.
    ///
    /// This gets culled after calling one of the `recv` methods.
    pub fn sources(&self) -> Vec<Arc<SmartChannelSource>> {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.retain(|r| r.is_connected());
        rx.iter().map(|r| r.source.clone()).collect()
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

    /// Maximum latency among all receivers (or 0, if none).
    pub fn latency_nanos(&self) -> u64 {
        re_tracing::profile_function!();
        let mut latency_nanos = 0;
        let rx = self.receivers.lock();
        for r in rx.iter() {
            latency_nanos = r.latency_nanos().max(latency_nanos);
        }
        latency_nanos
    }

    /// Sum queue length of all receivers.
    pub fn queue_len(&self) -> usize {
        re_tracing::profile_function!();
        let rx = self.receivers.lock();
        rx.iter().map(|r| r.len()).sum()
    }

    /// Blocks until a message is ready to be received,
    /// or we are empty.
    pub fn recv(&self) -> Result<SmartMessage<T>, RecvError> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        let mut sel = Select::new();
        for r in rx.iter() {
            sel.recv(&r.rx);
        }

        let oper = sel.select();
        let index = oper.index();
        oper.recv(&rx[index].rx).map_err(|_err| RecvError)
    }

    /// Returns immediately if there is nothing to receive.
    pub fn try_recv(&self) -> Option<(Arc<SmartChannelSource>, SmartMessage<T>)> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        if rx.is_empty() {
            return None;
        }

        let mut sel = Select::new();
        for r in rx.iter() {
            sel.recv(&r.rx);
        }

        let oper = sel.try_select().ok()?;
        let index = oper.index();
        if let Ok(msg) = oper.recv(&rx[index].rx) {
            return Some((rx[index].source.clone(), msg));
        }

        // Nothing ready to receive, but we must poll all receivers to update their `connected` status.
        // Why use `select` first? Because `select` is fair (random) when there is contention.
        for rx in rx.iter() {
            if let Ok(msg) = rx.try_recv() {
                return Some((rx.source.clone(), msg));
            }
        }

        None
    }

    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Option<(Arc<SmartChannelSource>, SmartMessage<T>)> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        rx.retain(|r| r.is_connected());
        if rx.is_empty() {
            return None;
        }

        let mut sel = Select::new();
        for r in rx.iter() {
            sel.recv(&r.rx);
        }

        let oper = sel.select_timeout(timeout).ok()?;
        let index = oper.index();
        if let Ok(msg) = oper.recv(&rx[index].rx) {
            return Some((rx[index].source.clone(), msg));
        }

        // Nothing ready to receive, but we must poll all receivers to update their `connected` status.
        // Why use `select` first? Because `select` is fair (random) when there is contention.
        for rx in rx.iter() {
            if let Ok(msg) = rx.try_recv() {
                return Some((rx.source.clone(), msg));
            }
        }

        None
    }
}

#[test]
fn test_receive_set() {
    use crate::{smart_channel, SmartMessageSource};

    let timeout = std::time::Duration::from_millis(100);

    let (tx_file, rx_file) = smart_channel::<i32>(
        SmartMessageSource::File("path".into()),
        SmartChannelSource::File("path".into()),
    );
    let (tx_sdk, rx_sdk) = smart_channel::<i32>(SmartMessageSource::Sdk, SmartChannelSource::Sdk);

    let set = ReceiveSet::default();

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(set.sources(), vec![]);

    set.add(rx_file);

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(
        set.sources(),
        vec![Arc::new(SmartChannelSource::File("path".into()))]
    );

    set.add(rx_sdk);

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(
        set.sources(),
        vec![
            Arc::new(SmartChannelSource::File("path".into())),
            Arc::new(SmartChannelSource::Sdk)
        ]
    );

    tx_sdk.send(42).unwrap();
    assert_eq!(set.try_recv().unwrap().0, Arc::new(SmartChannelSource::Sdk));

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(set.sources().len(), 2);

    drop(tx_sdk);

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(
        set.sources(),
        vec![Arc::new(SmartChannelSource::File("path".into()))]
    );

    drop(tx_file);

    assert_eq!(set.try_recv(), None);
    assert_eq!(set.recv_timeout(timeout), None);
    assert_eq!(set.sources(), vec![]);
}
