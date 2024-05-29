use super::StrokeWidth;

impl Default for StrokeWidth {
    fn default() -> Self {
        Self(8.0) // Reminder: these are ui points. Picking 1.0 is too small, 0.0 is invisible.
    }
}
