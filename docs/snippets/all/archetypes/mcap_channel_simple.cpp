// Log a simple MCAP channel definition.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_mcap_channel");
    rec.spawn().exit_on_failure();

    const std::vector<rerun::datatypes::Utf8Pair> metadata = {
        {"frame_id", "camera_link"},
        {"encoding", "bgr8"},
    };

    rec.log(
        "mcap/channels/camera",
        rerun::archetypes::McapChannel(1, "/camera/image", "cdr")
            .with_metadata(rerun::components::KeyValuePairs(metadata))
    );
}
