// Demonstrates the experimental state timeline view: logging state changes and customizing display.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec =
        rerun::RecordingStream("rerun_example_howto_state_timeline");
    rec.spawn().exit_on_failure();

    // region: state_config
    // Customize how each state value is displayed (label, color, visibility).
    // Log as static so the configuration applies for the entire recording.
    rec.log_static(
        "door",
        rerun::StateConfiguration()
            .with_values({"open", "closed"})
            .with_labels({"Open", "Closed"})
            .with_colors({0x4CAF50FF, 0xEF5350FF})
    );
    // endregion: state_config

    // region: log_changes
    // Log state transitions for two entities. Each call marks the start of a new state;
    // the previous state implicitly ends. The `/door` lane uses the `StateConfiguration`
    // above, while `/window` gets default styling (raw value as label, hashed color).
    rec.set_time_sequence("step", 0);
    rec.log("door", rerun::StateChange().with_state("open"));
    rec.log("window", rerun::StateChange().with_state("closed"));

    rec.set_time_sequence("step", 1);
    rec.log("door", rerun::StateChange().with_state("closed"));

    rec.set_time_sequence("step", 3);
    rec.log("window", rerun::StateChange().with_state("open"));

    rec.set_time_sequence("step", 4);
    rec.log("door", rerun::StateChange().with_state("open"));
    // endregion: log_changes
}
