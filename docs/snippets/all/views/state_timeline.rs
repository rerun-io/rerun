//! Use a blueprint to show a StateTimelineView.

use rerun::blueprint::{Blueprint, StateTimelineView};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        StateTimelineView::new("State Transitions").with_origin("/"),
    );

    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_state_timeline")
            .with_blueprint(blueprint)
            .spawn()?;

    rec.set_time_sequence("step", 0);
    rec.log("door", &rerun::StateChange::new().with_state("open"))?;

    rec.set_time_sequence("step", 1);
    rec.log("door", &rerun::StateChange::new().with_state("closed"))?;

    rec.set_time_sequence("step", 2);
    rec.log("door", &rerun::StateChange::new().with_state("open"))?;

    Ok(())
}
