//! Registrations with a catalog that are in progress, so that a caller can wait for them or
//! report them.

use std::collections::BTreeMap;
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
