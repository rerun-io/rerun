// Sets the recording properties.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_recording_properties");
    rec.spawn().exit_on_failure();

    rec.set_recording_name("My recording");
}
