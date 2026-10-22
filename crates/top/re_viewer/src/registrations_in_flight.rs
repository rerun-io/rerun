//! Registrations with a catalog that are in progress, so that a caller can wait for them or
//! report them.

use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Identifies one registration in progress. Later registrations get larger ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationId(u64);

/// A registration in progress.
#[derive(Clone, Debug)]
pub struct InFlightRegistration {
    /// The catalog that the file is being registered with.
    pub origin: re_uri::Origin,

    /// What is being registered, as a user would name it, e.g. a file path.
    pub name: String,
}

#[derive(Default)]
struct State {
    next_id: u64,
    in_flight: BTreeMap<RegistrationId, InFlightRegistration>,
}

/// The registrations in progress, shared by every clone.
///
/// This only keeps track of them. Registering the file is up to the caller.
#[derive(Clone, Default)]
pub struct RegistrationsInFlight(Arc<re_mutex::Mutex<State>>);

impl RegistrationsInFlight {
    /// Count `registration` as in progress until the guard is dropped.
    ///
    /// Drop the guard only once whatever the registration leads to is queued, such as loading
    /// the segment it produced: until then, this is all that shows it.
    pub fn begin(&self, registration: InFlightRegistration) -> RegistrationGuard {
        let mut state = self.0.lock();
        let id = RegistrationId(state.next_id);
        state.next_id += 1;
        state.in_flight.insert(id, registration);
        RegistrationGuard {
            registrations: self.clone(),
            id,
        }
    }

    /// Every registration in progress. Iterating it goes oldest first.
    pub fn snapshot(&self) -> BTreeMap<RegistrationId, InFlightRegistration> {
        self.0.lock().in_flight.clone()
    }
}

/// Counts a registration as in progress until dropped.
///
/// See [`RegistrationsInFlight::begin`].
#[must_use = "the registration only counts while the guard is alive"]
pub struct RegistrationGuard {
    registrations: RegistrationsInFlight,
    id: RegistrationId,
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        let Self { registrations, id } = self;
        registrations.0.lock().in_flight.remove(id);
    }
}

/// The dataset segments each local file was registered as, shared by every clone.
///
/// A recording loaded through the catalog only knows its segment, so this is what links it back
/// to the file path it was opened as.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
pub struct RegisteredFiles(Arc<re_mutex::Mutex<ahash::HashMap<PathBuf, Vec<re_uri::DatasetUri>>>>);

#[cfg(not(target_arch = "wasm32"))]
impl RegisteredFiles {
    /// Remember that `path` was registered as `segment`.
    ///
    /// Registering a regenerated file can produce a new segment while the old one is still open,
    /// so every segment is kept.
    pub fn insert(&self, path: &Path, segment: re_uri::DatasetUri) {
        let mut files = self.0.lock();
        let segments = files.entry(normalize(path)).or_default();
        if !segments.contains(&segment) {
            segments.push(segment);
        }
    }

    /// Every segment `path` was registered as.
    pub fn segments(&self, path: &Path) -> Vec<re_uri::DatasetUri> {
        self.0
            .lock()
            .get(&normalize(path))
            .cloned()
            .unwrap_or_default()
    }
}

/// The canonical form of `path`, so that every way of writing a file's path finds its segments.
///
/// A path that cannot be canonicalized, e.g. of a file that no longer exists, is only made
/// absolute.
#[cfg(not(target_arch = "wasm32"))]
fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .or_else(|_err| std::path::absolute(path))
        .unwrap_or_else(|_err| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(name: &str) -> InFlightRegistration {
        InFlightRegistration {
            origin: re_uri::Origin::test(),
            name: name.to_owned(),
        }
    }

    fn names(registrations: &RegistrationsInFlight) -> Vec<String> {
        registrations
            .snapshot()
            .into_values()
            .map(|registration| registration.name)
            .collect()
    }

    #[test]
    fn dropping_a_guard_removes_only_its_registration() {
        let registrations = RegistrationsInFlight::default();
        let first = registrations.begin(registration("a.rrd"));
        let second = registrations.begin(registration("b.rrd"));
        let third = registrations.clone().begin(registration("c.rrd"));

        drop(second);
        assert_eq!(names(&registrations), ["a.rrd", "c.rrd"]);

        drop(first);
        drop(third);
        assert_eq!(names(&registrations), Vec::<String>::new());
    }
}
