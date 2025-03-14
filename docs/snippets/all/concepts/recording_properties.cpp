// Sets the recording properties.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_recording_properties");
    rec.spawn().exit_on_failure();

    auto properties =
        rerun::archetypes::RecordingProperties().with_started(0).with_name("My recording (initial)"
        );
    rec.set_properties(properties);

    // Overwrites the name from above.
    rec.set_name("My recording");
}
