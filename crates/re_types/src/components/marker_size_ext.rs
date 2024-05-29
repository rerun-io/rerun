use super::MarkerSize;

impl Default for MarkerSize {
    fn default() -> Self {
        Self(8.0) // Reminder: these are ui points. Picking 1.0 is too small, 0.0 is invisible.
    }
}
