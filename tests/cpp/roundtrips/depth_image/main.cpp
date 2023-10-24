// Logs a `DepthImage` archetype for roundtrip checks.

#include <rerun/archetypes/depth_image.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_depth_image");
    rec.save(argv[1]).throw_on_failure();

    auto img = rerun::datatypes::TensorData({2, 3}, std::vector<uint8_t>{0, 1, 2, 3, 4, 5});

    rec.log("depth_image", rerun::archetypes::DepthImage(img).with_meter(1000.0));
}
