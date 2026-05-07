// Log a `Status` together with a `StatusConfiguration` that customizes its display.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_status_configuration");
    rec.spawn().exit_on_failure();

    // Configure how each raw status value is displayed (label, color, visibility).
    rec.log_static(
        "door",
        rerun::StatusConfiguration()
            .with_values({"open", "closed"})
            .with_labels({"Open", "Closed"})
            .with_colors({0x4CAF50FF, 0xEF5350FF})
    );

    rec.set_time_sequence("step", 0);
    rec.log("door", rerun::Status().with_status("open"));

    rec.set_time_sequence("step", 1);
    rec.log("door", rerun::Status().with_status("closed"));

    rec.set_time_sequence("step", 2);
    rec.log("door", rerun::Status().with_status("open"));
}
