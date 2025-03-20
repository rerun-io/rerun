// Sets the recording properties.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_recording_properties");
    rec.spawn().exit_on_failure();

    // Overwrites the name from above.
    rec.send_recording_name("My recording");

    // Overwrites the start time from above.
    rec.send_recording_start_time_nanos(42);

    auto points = rerun::Points3D({{1.0f, 0.1, 1.0}});
    rec.send_property("camera_left", points);
}
