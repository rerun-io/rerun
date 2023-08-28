use std::sync::Arc;

use crossbeam::channel::Select;
use parking_lot::Mutex;

use crate::{Receiver, RecvError, SmartChannelSource, SmartMessage};

/// A set of [`Receiver`]s.
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

    /// Any receviers left?
    pub fn is_connected(&self) -> bool {
        !self.is_empty()
    }

    /// No receivers left.
    pub fn is_empty(&self) -> bool {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.retain(|r| r.is_connected());
        rx.is_empty()
    }

    pub fn latency_ns(&self) -> u64 {
        re_tracing::profile_function!();
        let mut latency_ns = 0;
        let rx = self.receivers.lock();
        for r in rx.iter() {
            latency_ns = r.latency_ns().max(latency_ns);
        }
        latency_ns
    }

    pub fn queue_len(&self) -> usize {
        re_tracing::profile_function!();
        let rx = self.receivers.lock();
        rx.iter().map(|r| r.len()).sum()
    }

    pub fn sources(&self) -> Vec<Arc<SmartChannelSource>> {
        re_tracing::profile_function!();
        let mut rx = self.receivers.lock();
        rx.retain(|r| r.is_connected());
        rx.iter().map(|r| r.source.clone()).collect()
    }

    /// Blocks until a message is ready to be received,
    /// or we are empty.
    pub fn recv(&self) -> Result<SmartMessage<T>, RecvError> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        loop {
            rx.retain(|r| r.is_connected());
            if rx.is_empty() {
                return Err(RecvError);
            }

            let mut sel = Select::new();
            for r in rx.iter() {
                sel.recv(&r.rx);
            }

            let oper = sel.select();
            let index = oper.index();
            if let Ok(msg) = oper.recv(&rx[index].rx) {
                return Ok(msg);
            }
        }
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
            Some((rx[index].source.clone(), msg))
        } else {
            None
        }
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
            Some((rx[index].source.clone(), msg))
        } else {
            None
        }
    }
}
