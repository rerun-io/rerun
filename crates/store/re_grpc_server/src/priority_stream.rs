use std::pin::Pin;
use std::task::{Context, Poll};

use tokio_stream::Stream;

/// Merges multiple streams, favoring earlier streams over later ones.
///
/// The merged stream terminates when all streams are exhausted.
/// Streams are polled in priority order - if a higher priority stream has items ready,
/// those are returned before checking lower priority streams.
pub struct PriorityMerge<T> {
    /// Streams in priority order (highest priority first).
    /// Exhausted streams are removed from the vec.
    streams: Vec<Pin<Box<dyn Stream<Item = T> + Send>>>,
}

impl<T> PriorityMerge<T> {
    pub fn new<S1, S2>(high_priority: S1, low_priority: S2) -> Self
    where
        S1: Stream<Item = T> + Send + 'static,
        S2: Stream<Item = T> + Send + 'static,
    {
        Self {
            streams: vec![Box::pin(high_priority), Box::pin(low_priority)],
        }
    }
}

impl<T> Stream for PriorityMerge<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut result = None;

        self.streams.retain_mut(|stream| {
            if result.is_some() {
                // Already found an item, keep remaining streams without polling
                return true;
            }

            match stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    result = Some(item);
                    true // Keep this stream
                }
                Poll::Ready(None) => false, // Remove exhausted stream
                Poll::Pending => true,      // Keep pending stream
            }
        });

        match result {
            Some(item) => Poll::Ready(Some(item)),
            None if self.streams.is_empty() => Poll::Ready(None),
            None => Poll::Pending,
        }
    }
}
