use std::time::Duration;

use re_analytics::{Analytics, Event};

fn input_filled_event(body: String) -> Event {
    Event::append("input_filled".into()).with_prop("body".into(), body)
}

fn main() {
    tracing_subscriber::fmt::init(); // log to stdout

    let analytics = Analytics::new(
        Duration::from_secs(3),
        [
            (
                "application_id".into(),
                "end_to_end_example".to_owned().into(),
            ),
            (
                "recording_id".into(),
                uuid::Uuid::new_v4().to_string().into(),
            ),
        ]
        .into(),
    )
    .unwrap();

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
