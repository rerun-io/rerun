use crossbeam::channel::Receiver;
use re_async::{AsyncRuntimeHandle, WasmNotSend};

/// A handle to an object that is requested asynchronously.
///
/// Note: this object cannot be [`Clone`] because it uses a one-shot channel to track completion of
/// the async operation.
#[derive(Debug)]
pub enum RequestedObject<T: Send + 'static, E: Send + 'static> {
    NotYetRequested {
        previous: Option<T>,
    },
    Pending {
        rx: Receiver<Result<T, E>>,
        previous: Option<T>,
        // TODO(grtlr): consider adding a timestamp for when the request was initiated.
        // This would allow us to show a loading spinner only after a certain amount of
        // time has passed, to avoid further UI flickers.
    },
    Completed(T),
    Unavailable {
        err: E,
        previous: Option<T>,
    },
}

impl<T: Send + 'static, E: Send + 'static> Default for RequestedObject<T, E> {
    fn default() -> Self {
        Self::NotYetRequested { previous: None }
    }
}

/// A value that has to be fetched from the server holding it.
///
/// Nothing is fetched until something asks for the value, so each value of a larger structure
/// is fetched on its own, whenever it is first needed.
///
/// This is [`RequestedObject`] minus the channel the answer arrives on, which is why the two are
/// separate: the channel stays in the [`RequestedObject`], since a value handed to the UI has to be
/// cheap to hand out, and a cloned receiver would consume the answer out from under us.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerValue<T, E> {
    /// We are waiting for the value.
    Pending { previous: Option<T> },

    /// The server told us.
    Completed(T),

    /// We asked, but the server couldn't tell us. The reason is in the log.
    Unavailable { previous: Option<T>, err: E },
}

impl<T, E> ServerValue<T, E> {
    /// The value, if the server has told us.
    pub fn get(&self) -> Option<&T> {
        match self {
            Self::Completed(value) => Some(value),
            Self::Pending { previous } | Self::Unavailable { previous, .. } => previous.as_ref(),
        }
    }

    pub fn get_err(&self) -> Option<&E> {
        match self {
            Self::Pending { .. } | Self::Completed(_) => None,
            Self::Unavailable { err, .. } => Some(err),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ServerValue<U, E> {
        match self {
            Self::Pending { previous } => ServerValue::Pending {
                previous: previous.map(f),
            },
            Self::Completed(t) => ServerValue::Completed(f(t)),
            Self::Unavailable { err, previous } => ServerValue::Unavailable {
                err,
                previous: previous.map(f),
            },
        }
    }
}

impl<T: Clone, E: Clone> ServerValue<&T, &E> {
    pub fn cloned(&self) -> ServerValue<T, E> {
        match self {
            Self::Pending { previous } => ServerValue::Pending {
                previous: previous.cloned(),
            },
            Self::Completed(v) => ServerValue::Completed((*v).clone()),
            Self::Unavailable { previous, err } => ServerValue::Unavailable {
                previous: previous.cloned(),
                err: (*err).clone(),
            },
        }
    }
}

impl<T: Send + 'static, E: std::fmt::Debug + Send + 'static> RequestedObject<T, E> {
    /// Run `fetch` in the background, unless something asked for the object already.
    ///
    /// `fetch` is only called if we actually need it, so this is cheap to call every frame.
    /// The UI is repainted once the object arrives.
    pub fn request<F>(
        &mut self,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        fetch: impl FnOnce() -> F,
    ) where
        T: std::fmt::Debug,
        F: std::future::Future<Output = Result<T, E>> + WasmNotSend + 'static,
    {
        if matches!(self, Self::NotYetRequested { .. }) {
            let previous = std::mem::take(self).take_latest();
            let (tx, rx) = re_quota_channel::create_crossbeam_channel(1);
            *self = Self::Pending { rx, previous };

            let fetch = fetch();
            let egui_ctx = egui_ctx.clone();
            runtime.spawn_future(async move {
                //TODO(#9836): implement cancellation using another channel (see `make_future_send`)
                let result = fetch.await;
                re_quota_channel::send_crossbeam(&tx, result).ok();
                egui_ctx.request_repaint();
            });
        }
    }

    /// The value, starting the fetch for it if nothing has asked for it yet.
    pub fn request_value<F>(
        &mut self,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        fetch: impl FnOnce() -> F,
    ) -> ServerValue<T, E>
    where
        F: std::future::Future<Output = Result<T, E>> + WasmNotSend + 'static,
        T: std::fmt::Debug + Clone,
        E: Clone,
    {
        self.request(runtime, egui_ctx, fetch);
        self.poll();

        self.value().cloned()
    }

    /// Take the latest available object, if any.
    pub fn take_latest(self) -> Option<T> {
        match self {
            Self::NotYetRequested { previous }
            | Self::Pending { previous, .. }
            | Self::Unavailable { previous, .. } => previous,
            Self::Completed(result) => Some(result),
        }
    }

    /// Update our state if the fetch has completed since we last looked.
    pub fn poll(&mut self) {
        if let Self::Pending { rx, previous } = self
            && let Ok(result) = rx.try_recv()
        {
            *self = match result {
                Ok(result) => Self::Completed(result),
                Err(err) => Self::Unavailable {
                    err,
                    previous: previous.take(),
                },
            }
        }
    }

    /// Get a reference to the latest available object.
    pub fn get(&self) -> Option<&T> {
        self.value().get().copied()
    }

    pub fn value(&self) -> ServerValue<&T, &E> {
        match self {
            Self::NotYetRequested { previous } | Self::Pending { previous, rx: _ } => {
                ServerValue::Pending {
                    previous: previous.as_ref(),
                }
            }
            Self::Completed(value) => ServerValue::Completed(value),
            Self::Unavailable { err, previous } => ServerValue::Unavailable {
                err,
                previous: previous.as_ref(),
            },
        }
    }

    /// Fetch again the next time something asks, keeping the latest object until then.
    pub fn refresh(&mut self) {
        let previous = std::mem::take(self).take_latest();

        *self = Self::NotYetRequested { previous }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test is to ensure you think twice before deriving `Clone` for [`RequestedObject`] (see
    /// docstring for the background).
    #[test]
    fn requested_object_not_clone() {
        static_assertions::assert_not_impl_any!(RequestedObject<usize, ()>: Clone);
    }
}
