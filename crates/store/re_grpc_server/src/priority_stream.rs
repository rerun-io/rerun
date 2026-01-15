use std::pin::Pin;
use std::task::{Context, Poll};

use tokio_stream::Stream;

/// Merges two streams, favoring one above the other.
///
/// The merged stream only terminates when the high priority stream terminates.
/// When the low priority stream is exhausted, we continue polling only the high priority stream.
pub struct PriorityMerge<S1, S2> {
    high_priority: Pin<Box<S1>>,
    low_priority: Pin<Box<S2>>,
    low_priority_done: bool,
}

impl<S1, S2> PriorityMerge<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
{
    pub fn new(high_priority: S1, low_priority: S2) -> Self {
        Self {
            high_priority: Box::pin(high_priority),
            low_priority: Box::pin(low_priority),
            low_priority_done: false,
        }
    }
}

impl<S1, S2> Stream for PriorityMerge<S1, S2>
where
    S1: Stream,
    S2: Stream<Item = S1::Item>,
{
    type Item = S1::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.high_priority.as_mut().poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
            Poll::Ready(None) => Poll::Ready(None), // High priority done = stream done
            Poll::Pending => {
                if self.low_priority_done {
                    Poll::Pending
                } else {
                    match self.low_priority.as_mut().poll_next(cx) {
                        Poll::Ready(Some(item)) => Poll::Ready(Some(item)),
                        Poll::Ready(None) => {
                            self.low_priority_done = true;
                            Poll::Pending
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
            }
        }
    }
}
