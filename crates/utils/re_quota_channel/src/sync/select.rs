//! Selection utilities for quota channels.

use super::{Receiver, RecvError};

pub use crossbeam::channel::{SelectTimeoutError, TrySelectError};

/// Wait on two receivers and execute whichever branch becomes ready first.
///
/// This is a simplified version of `crossbeam::select!` that only supports
/// two `recv` operations. It properly handles the byte accounting by calling
/// `manual_on_receive` after each successful receive.
///
/// **Note:** This macro blocks until one of the receivers is ready, so it is
/// **not available on wasm32**.
///
/// # Syntax
///
/// ```ignore
/// select! {
///     recv(rx1) -> result => { /* handle result */ },
///     recv(rx2) -> result => { /* handle result */ },
/// }
/// ```
///
/// # Example
///
/// ```
/// use re_quota_channel::{channel, select};
///
/// let (tx1, rx1) = channel::<i32>("chan1", 1024);
/// let (tx2, rx2) = channel::<String>("chan2", 1024);
///
/// tx1.send_with_size(42, 8).unwrap();
///
/// select! {
///     recv(rx1) -> res => {
///         assert_eq!(res.unwrap(), 42);
///     },
///     recv(rx2) -> res => {
///         panic!("unexpected");
///     },
/// }
/// ```
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! select {
    // Without comma separator (for block syntax) - forward to comma version
    (
        recv($rx1:expr) -> $res1:tt => $body1:block
        recv($rx2:expr) -> $res2:tt => $body2:block
    ) => {
        $crate::select! {
            recv($rx1) -> $res1 => $body1,
            recv($rx2) -> $res2 => $body2,
        }
    };

    // With comma separator - the main implementation
    (
        recv($rx1:expr) -> $res1:tt => $body1:tt,
        recv($rx2:expr) -> $res2:tt => $body2:tt $(,)?
    ) => {{
        let rx1 = &$rx1;
        let rx2 = &$rx2;

        let mut sel = $crate::Select::new();
        sel.recv(rx1);
        sel.recv(rx2);

        let oper = sel.select();
        match oper.index() {
            0 => {
                let $res1 = oper.recv(rx1);
                $body1
            }
            1 => {
                let $res2 = oper.recv(rx2);
                $body2
            }
            _ => unreachable!(),
        }
    }};
}

// ----------------------------------------------------------------------------

/// A dynamic selection interface for receiving from multiple quota channels.
///
/// This wraps [`crossbeam::channel::Select`] and provides proper byte accounting.
///
/// **Note:** Blocking select operations are **not available on wasm32**.
/// Use [`Select::try_select`] for non-blocking operations on wasm.
///
/// # Example
///
/// ```
/// use re_quota_channel::{channel, Select};
///
/// let (tx1, rx1) = channel::<i32>("chan1", 1024);
/// let (tx2, rx2) = channel::<i32>("chan2", 1024);
///
/// tx1.send_with_size(42, 8).unwrap();
///
/// let mut sel = Select::new();
/// sel.recv(&rx1);
/// sel.recv(&rx2);
///
/// let oper = sel.select();
/// let index = oper.index();
/// match index {
///     0 => {
///         let msg = oper.recv(&rx1).unwrap();
///         assert_eq!(msg, 42);
///     }
///     1 => {
///         let _msg = oper.recv(&rx2).unwrap();
///     }
///     _ => unreachable!(),
/// }
/// ```
pub struct Select<'a> {
    inner: crossbeam::channel::Select<'a>,
}

impl<'a> Select<'a> {
    /// Creates a new `Select`.
    pub fn new() -> Self {
        Self {
            inner: crossbeam::channel::Select::new(),
        }
    }

    /// Adds a receive operation to the selection.
    ///
    /// Returns the index of the added operation.
    pub fn recv<T>(&mut self, rx: &'a Receiver<T>) -> usize {
        self.inner.recv(rx.inner())
    }

    /// Blocks until one of the registered operations becomes ready.
    ///
    /// Returns a `SelectedOperation` that can be used to complete the receive.
    ///
    /// **Note:** Not available on wasm32 - use [`Self::try_select`] instead.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn select(&mut self) -> SelectedOperation<'a> {
        SelectedOperation {
            inner: self.inner.select(),
        }
    }

    /// Attempts to find a ready operation without blocking.
    ///
    /// Returns a `SelectedOperation` if one is ready, or an error otherwise.
    pub fn try_select(&mut self) -> Result<SelectedOperation<'a>, TrySelectError> {
        self.inner
            .try_select()
            .map(|inner| SelectedOperation { inner })
    }

    /// Blocks until one of the registered operations becomes ready or times out.
    ///
    /// Returns a `SelectedOperation` if one becomes ready within the timeout.
    ///
    /// **Note:** Not available on wasm32 - use [`Self::try_select`] instead.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn select_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<SelectedOperation<'a>, SelectTimeoutError> {
        self.inner
            .select_timeout(timeout)
            .map(|inner| SelectedOperation { inner })
    }
}

impl Default for Select<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// A selected operation that is ready to be completed.
///
/// This is returned by [`Select::select`], [`Select::try_select`], and [`Select::select_timeout`].
pub struct SelectedOperation<'a> {
    inner: crossbeam::channel::SelectedOperation<'a>,
}

impl SelectedOperation<'_> {
    /// Returns the index of the selected operation.
    ///
    /// This corresponds to the order in which receivers were added to the [`Select`].
    pub fn index(&self) -> usize {
        self.inner.index()
    }

    /// Completes the receive operation.
    ///
    /// This properly handles byte accounting by calling `manual_on_receive`.
    pub fn recv<T>(self, rx: &Receiver<T>) -> Result<T, RecvError> {
        self.inner.recv(rx.inner()).map(|sized| {
            rx.manual_on_receive(sized.size_bytes);
            sized.msg
        })
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::sync::channel;

    use super::*;

    #[test]
    fn test_select_macro() {
        let (tx1, rx1) = channel::<i32>("chan1".to_owned(), 1000);
        let (tx2, rx2) = channel::<String>("chan2".to_owned(), 1000);

        // Send to first channel
        tx1.send_with_size(42, 8).unwrap();

        // Select should return from first channel
        crate::select! {
            recv(rx1) -> res => {
                assert_eq!(res.unwrap(), 42);
            },
            recv(rx2) -> _res => {
                panic!("expected rx1, got rx2");
            },
        }

        // Byte accounting should be updated
        assert_eq!(rx1.current_bytes(), 0);

        // Now send to second channel
        tx2.send_with_size("hello".to_owned(), 100).unwrap();

        // Select should return from second channel
        crate::select! {
            recv(rx1) -> _res => {
                panic!("expected rx2, got rx1");
            },
            recv(rx2) -> res => {
                assert_eq!(res.unwrap(), "hello");
            },
        }

        // Byte accounting should be updated
        assert_eq!(rx2.current_bytes(), 0);
    }

    #[test]
    fn test_select_struct() {
        let (tx1, rx1) = channel::<i32>("chan1".to_owned(), 1000);
        let (tx2, rx2) = channel::<String>("chan2".to_owned(), 1000);

        // Send to first channel
        tx1.send_with_size(42, 8).unwrap();

        // Use Select struct
        let mut sel = Select::new();
        sel.recv(&rx1);
        sel.recv(&rx2);

        let oper = sel.select();
        assert_eq!(oper.index(), 0); // First receiver should be ready
        let msg = oper.recv(&rx1).unwrap();
        assert_eq!(msg, 42);

        // Byte accounting should be updated
        assert_eq!(rx1.current_bytes(), 0);

        // Now send to second channel and test try_select
        tx2.send_with_size("hello".to_owned(), 100).unwrap();

        let mut sel = Select::new();
        sel.recv(&rx1);
        sel.recv(&rx2);

        let oper = sel.try_select().unwrap();
        assert_eq!(oper.index(), 1); // Second receiver should be ready
        let msg = oper.recv(&rx2).unwrap();
        assert_eq!(msg, "hello");

        // Byte accounting should be updated
        assert_eq!(rx2.current_bytes(), 0);

        // Test try_select when nothing is ready
        let mut sel = Select::new();
        sel.recv(&rx1);
        sel.recv(&rx2);

        assert!(sel.try_select().is_err());
    }
}
