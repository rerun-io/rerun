use crossbeam::channel::Select;
use parking_lot::Mutex;

use crate::{Receiver, RecvError, SmartMessage};

/// A set of [`Receiver`]s.
pub struct ReceiveSet<T: Send> {
    receivers: Mutex<Vec<Receiver<T>>>,
}

impl<T: Send> ReceiveSet<T> {
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
    pub fn try_recv(&self) -> Option<SmartMessage<T>> {
        re_tracing::profile_function!();

        let mut rx = self.receivers.lock();

        loop {
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
                return Some(msg);
            }
        }
    }
}
