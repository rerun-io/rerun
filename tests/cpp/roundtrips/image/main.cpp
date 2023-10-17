// Logs an `Image` archetype for roundtrip checks.

#include <rerun/archetypes/image.hpp>
#include <rerun/recording_stream.hpp>

int main(int argc, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_image");
    rec.save(argv[1]).throw_on_failure();

    // 2x3x3 image. Red channel = x. Green channel = y. Blue channel = 128.
    auto img = rerun::datatypes::TensorData(
        {3, 2, 3},
        std::vector<uint8_t>{0, 0, 128, 1, 0, 128, 2, 0, 128, 0, 1, 128, 1, 1, 128, 2, 1, 128}
    );
    rec.log("image", rerun::archetypes::Image(img));

    // TODO(andreas):
    // 4x5 mono image. Pixel = x * y * 123.4
    // auto img = rerun::datatypes::TensorData({4, 5}, /* ?? */);

    // rec.log("image_f16", rerun::archetypes::Image(img));
}
