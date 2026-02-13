use re_analytics::{Analytics, AnalyticsEvent, Event, Properties};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    re_log::setup_logging();

    let analytics = Analytics::global_or_init().expect("Failed to initialize analytics");
    let application_id = "end_to_end_example".to_owned();
    let recording_id = uuid::Uuid::new_v4().simple().to_string();

    println!("any non-empty line written here will be sent as an analytics datapoint");
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

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
    fn serialize(self, event: &mut AnalyticsEvent) {
        let Self {
            application_id,
            recording_id,
            body,
        } = self;

        event.insert("application_id", application_id);
        event.insert("recording_id", recording_id);
        event.insert("body", body);
    }
}
