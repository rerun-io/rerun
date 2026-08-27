//! Reorder buffer turning frames from decode order into presentation order.
//!
//! Shared by the backends: the Vulkan backend keys frames by picture order count,
//! the `VideoToolbox` backend will key them by presentation timestamp.

/// Buffers decoded frames until their position in presentation order is settled.
///
/// A frame's position is settled once more than `reorder_delay` frames are pending
/// (at most that many frames can precede a frame in decode order but follow it in
/// presentation order), or when an IDR frame arrives (no frame is presented across
/// one in the other order).
pub struct ReorderBuffer<F> {
    /// Pending frames, sorted ascending by key.
    pending: Vec<(i64, F)>,
}

impl<F> ReorderBuffer<F> {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Buffers one decoded frame under its presentation order `key`,
    /// appending every frame whose order is now settled to `out`.
    pub fn push(
        &mut self,
        key: i64,
        is_idr: bool,
        frame: F,
        reorder_delay: usize,
        out: &mut Vec<F>,
    ) {
        if is_idr {
            self.flush(out);
        }

        let index = self.pending.partition_point(|(pending, _)| *pending <= key);
        self.pending.insert(index, (key, frame));

        while self.pending.len() > reorder_delay {
            out.push(self.pending.remove(0).1);
        }
    }

    /// Emits all pending frames in presentation order: the stream ended.
    pub fn flush(&mut self, out: &mut Vec<F>) {
        out.extend(self.pending.drain(..).map(|(_, frame)| frame));
    }

    /// Drops all pending frames for a seek.
    pub fn reset(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::ReorderBuffer;

    fn push(buffer: &mut ReorderBuffer<i64>, key: i64, is_idr: bool, delay: usize) -> Vec<i64> {
        let mut out = Vec::new();
        buffer.push(key, is_idr, key, delay, &mut out);
        out
    }

    /// With no reordering in the stream every frame comes back immediately.
    #[test]
    fn zero_delay_passes_through() {
        let mut buffer = ReorderBuffer::new();
        for key in [0, 2, 4, 6] {
            assert_eq!(push(&mut buffer, key, key == 0, 0), vec![key]);
        }
    }

    /// B-frames arrive after the future frame they reference. With a delay of one,
    /// each emission waits for one more frame, putting the output in key order.
    #[test]
    fn reorders_b_frames() {
        let mut buffer = ReorderBuffer::new();
        assert_eq!(push(&mut buffer, 0, true, 1), Vec::<i64>::new());
        assert_eq!(push(&mut buffer, 4, false, 1), vec![0]);
        assert_eq!(push(&mut buffer, 2, false, 1), vec![2]);
        assert_eq!(push(&mut buffer, 8, false, 1), vec![4]);
        assert_eq!(push(&mut buffer, 6, false, 1), vec![6]);

        let mut out = Vec::new();
        buffer.flush(&mut out);
        assert_eq!(out, vec![8]);
    }

    /// An IDR frame settles everything before it, even while the buffer
    /// holds fewer frames than the reorder delay.
    #[test]
    fn idr_flushes_pending() {
        let mut buffer = ReorderBuffer::new();
        assert_eq!(push(&mut buffer, 0, true, 4), Vec::<i64>::new());
        assert_eq!(push(&mut buffer, 4, false, 4), Vec::<i64>::new());
        assert_eq!(push(&mut buffer, 2, false, 4), Vec::<i64>::new());
        // Keys restart at zero after an IDR, flushing keeps the groups apart.
        assert_eq!(push(&mut buffer, 0, true, 4), vec![0, 2, 4]);
        assert_eq!(push(&mut buffer, 2, false, 4), Vec::<i64>::new());

        let mut out = Vec::new();
        buffer.flush(&mut out);
        assert_eq!(out, vec![0, 2]);
    }

    /// A seek drops the pending frames instead of emitting them.
    #[test]
    fn reset_drops_pending() {
        let mut buffer = ReorderBuffer::new();
        assert_eq!(push(&mut buffer, 0, true, 4), Vec::<i64>::new());
        assert_eq!(push(&mut buffer, 4, false, 4), Vec::<i64>::new());
        buffer.reset();

        let mut out = Vec::new();
        buffer.flush(&mut out);
        assert!(out.is_empty());
    }
}
