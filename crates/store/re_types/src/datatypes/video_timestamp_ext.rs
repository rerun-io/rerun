use super::{VideoTimeMode, VideoTimestamp};

impl VideoTimestamp {
    /// Create new timestamp from nanoseconds since video start.
    #[inline]
    pub fn new_nanoseconds(nanos: i64) -> Self {
        Self {
            video_time: nanos,
            time_mode: VideoTimeMode::Nanoseconds,
        }
    }
}

impl Default for VideoTimestamp {
    fn default() -> Self {
        Self {
            video_time: 0,
            time_mode: VideoTimeMode::Nanoseconds,
        }
    }
}
