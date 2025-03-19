// Sets the recording properties.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_recording_properties");
    rec.spawn().exit_on_failure();

    auto properties =
        rerun::RecordingProperties().with_start_time(0).with_name("My recording (initial)");
    rec.set_properties(properties);

    // Overwrites the name from above.
    rec.set_name("My recording");

    // Overwrites the start time from above.
    rec.set_start_time_nanos(42);

    auto points = rerun::Points3D({{1.0f, 0.1, 1.0}});
    rec.set_properties_with_prefix("cameras/left", points);
}
