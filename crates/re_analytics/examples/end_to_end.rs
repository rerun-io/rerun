use std::time::Duration;

use re_analytics::{Analytics, Event};

fn input_filled_event(body: String) -> Event {
    Event::append("input_filled").with_prop("body", body)
}

fn main() {
    re_log::setup_logging();

    let mut analytics = Analytics::new(Duration::from_secs(3)).unwrap();
    analytics.register_append_property("application_id", "end_to_end_example");
    analytics.register_append_property("recording_id", uuid::Uuid::new_v4().to_string());

    println!("any non-empty line written here will be sent as an analytics datapoint");
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();
        if !input.is_empty() {
            analytics.record(input_filled_event(input.to_owned()));
        }
    }
}
