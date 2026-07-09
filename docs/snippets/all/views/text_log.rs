//! Use a blueprint to show a text log.

use rerun::blueprint::{
    archetypes as blueprint_archetypes, datatypes as blueprint_datatypes,
    Blueprint, TextLogView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        TextLogView::new("Text Logs")
            .with_origin("/log")
            .with_columns(
                blueprint_archetypes::TextLogColumns::new()
                    .with_timeline_columns([blueprint_datatypes::TimelineColumn {
                        timeline: "time".into(),
                        visible: true.into(),
                    }])
                    .with_text_log_columns([
                        blueprint_datatypes::TextLogColumn {
                            kind: blueprint_datatypes::TextLogColumnKind::LogLevel,
                            visible: true.into(),
                        },
                        blueprint_datatypes::TextLogColumn {
                            kind: blueprint_datatypes::TextLogColumnKind::EntityPath,
                            visible: true.into(),
                        },
                        blueprint_datatypes::TextLogColumn {
                            kind: blueprint_datatypes::TextLogColumnKind::Body,
                            visible: true.into(),
                        },
                    ]),
            )
            .with_rows(
                blueprint_archetypes::TextLogRows::new()
                    .with_filter_by_log_level(["INFO", "WARN", "ERROR"]),
            )
            .with_format_options(
                blueprint_archetypes::TextLogFormat::new()
                    .with_monospace_body(false),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_log")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.set_time_sequence("time", 0);
    rec.log(
        "log/status",
        &rerun::TextLog::new("Application started.").with_level("INFO"),
    )?;
    rec.set_time_sequence("time", 5);
    rec.log(
        "log/other",
        &rerun::TextLog::new("A warning.").with_level("WARN"),
    )?;

    for i in 0..10 {
        rec.set_time_sequence("time", i);
        rec.log(
            "log/status",
            &rerun::TextLog::new(format!("Processing item {i}."))
                .with_level("INFO"),
        )?;
    }

    Ok(())
}
