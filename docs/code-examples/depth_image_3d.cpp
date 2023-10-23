// Create and log a depth image.

#include <rerun.hpp>

#include <algorithm>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_depth_image");
    rec.connect().throw_on_failure();

    // Create a synthetic depth image.
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    for (auto y = 50; y < 150; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 50, 100, 20000);
    }
    for (auto y = 130; y < 180; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 100, 180, 45000);
    }

    // If we log a pinhole camera model, the depth gets automatically back-projected to 3D
    rec.log(
        "world/camera",
        rerun::Pinhole::focal_length_and_resolution(
            {20.0f, 20.0f},
            {static_cast<float>(WIDTH), static_cast<float>(HEIGHT)}
        )
    );

    rec.log(
        "world/camera/depth",
        rerun::DepthImage({HEIGHT, WIDTH}, std::move(data)).with_meter(10000.0)
    );
}
