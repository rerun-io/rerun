use std::time::Duration;

/// Half-open `[start, end)` window into a video stream, as durations since the start of the video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeWindow {
    /// Inclusive lower bound. Private so `end <= start` stays unrepresentable.
    start: Duration,

    /// Exclusive upper bound; always greater than [`Self::start`].
    end: Duration,
}

impl TimeWindow {
    /// `None` if `end <= start` — inverted and empty windows are unrepresentable.
    pub fn new(start: Duration, end: Duration) -> Option<Self> {
        (start < end).then_some(Self { start, end })
    }

    #[inline]
    pub fn start(&self) -> Duration {
        self.start
    }

    #[inline]
    pub fn end(&self) -> Duration {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverted_and_empty_windows_are_rejected() {
        let s = Duration::from_secs;
        assert!(TimeWindow::new(s(1), s(2)).is_some());
        assert!(TimeWindow::new(s(1), s(1)).is_none());
        assert!(TimeWindow::new(s(2), s(1)).is_none());
    }
}
