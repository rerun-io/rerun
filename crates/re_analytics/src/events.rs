use time::OffsetDateTime;

use crate::Event;

// TODO: explain why this file exists (audit purposes)

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
