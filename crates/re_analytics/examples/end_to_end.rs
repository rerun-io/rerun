use std::time::Duration;

use re_analytics::Event;
use re_analytics::Properties;
use re_analytics::{Analytics, AnalyticsEvent};

fn main() {
    re_log::setup_logging();

    let analytics = Analytics::new(Duration::from_secs(3)).unwrap();
    let application_id = "end_to_end_example".to_owned();
    let recording_id = uuid::Uuid::new_v4().to_string();

    println!("any non-empty line written here will be sent as an analytics datapoint");
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();
        if !input.is_empty() {
            analytics.record(InputFilled {
                application_id: application_id.clone(),
                recording_id: recording_id.clone(),
                body: input.to_owned(),
            });
        }
    }
}

struct InputFilled {
    application_id: String,
    recording_id: String,
    body: String,
}

impl Event for InputFilled {
    const NAME: &'static str = "input_filled";
}

impl Properties for InputFilled {
    fn serialize(&self, event: &mut AnalyticsEvent) {
        event.insert("application_id", self.application_id.clone());
        event.insert("recording_id", self.recording_id.clone());
        event.insert("body", self.body.clone());
    }
}
