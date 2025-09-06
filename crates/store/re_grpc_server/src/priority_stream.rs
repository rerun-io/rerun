use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio_stream::Stream;

/// Merges two streams, favoring one above the other
pub struct PriorityMerge<S1, S2> {
    high_priority: Pin<Box<S1>>,
    low_priority: Pin<Box<S2>>,
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
        if let Poll::Ready(item) = self.high_priority.as_mut().poll_next(cx) {
            Poll::Ready(item)
        } else {
            self.low_priority.as_mut().poll_next(cx)
        }
    }
}
