use time::OffsetDateTime;

use crate::Event;

// All of our telemtry events are defined in this one file, to facilitate auditing.

// ---

impl Event {
    pub fn viewer_opened() -> Self {
        // TODO: crate version and such
        Self {
            time_utc: OffsetDateTime::now_utc(),
            name: "viewer_opened".into(),
            props: Default::default(),
        }
    }
}
