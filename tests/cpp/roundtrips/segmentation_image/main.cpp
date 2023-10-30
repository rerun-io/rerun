// Logs a `SegmentationImage` archetype for roundtrip checks.

#include <rerun/archetypes/segmentation_image.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_segmentation_image");
    rec.save(argv[1]).exit_on_failure()

        // 3x2 image. Each pixel is incremented down each row
        auto img = rerun::datatypes::TensorData({2, 3}, std::vector<uint8_t>{0, 1, 2, 3, 4, 5});

    rec.log("segmentation_image", rerun::archetypes::SegmentationImage(img));
}
