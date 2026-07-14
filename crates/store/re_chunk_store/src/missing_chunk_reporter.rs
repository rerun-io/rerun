use std::sync::atomic::{AtomicBool, Ordering};

/// Used to report if we missed some chunks.
///
/// A missing chunk means there is a virtual chunk that is missing its physical backing.
/// This can be the result of:
/// * A chunk has not yet been loaded from Redap
/// * A chunk was loaded, but then evicted due to GC.
///
/// In the second case, we may not be connected to a Redap server.
/// Depending on whether we are, the caller should either show a loading indicator,
/// or some other warning.
///
/// Missing a chunk is usually not a hard error, but it is a warning,
/// hence the use of the reporter pattern.
#[derive(Default)]
#[must_use = "You should report missing chunks"]
pub struct MissingChunkReporter(AtomicBool);

impl Clone for MissingChunkReporter {
    fn clone(&self) -> Self {
        Self(AtomicBool::new(self.0.load(Ordering::Relaxed)))
    }
}

impl MissingChunkReporter {
    pub fn new(any_missing: bool) -> Self {
        Self(AtomicBool::new(any_missing))
    }

    pub fn report_missing_chunk(&self) {
        // In the future we may accumulate the chunk ids here.
        self.0.store(true, Ordering::Relaxed);
    }

    // No warnings emitted
    pub fn is_empty(&self) -> bool {
        !self.any_missing()
    }

    pub fn any_missing(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl std::iter::Sum for MissingChunkReporter {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let sum = Self::default();
        for r in iter {
            if r.any_missing() {
                sum.report_missing_chunk();
            }
        }
        sum
    }
}
