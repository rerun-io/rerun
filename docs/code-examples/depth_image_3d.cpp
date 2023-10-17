// Create and log a depth image.

#include <rerun.hpp>

#include <algorithm>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_depth_image");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // Create a synthetic depth image.
    const int HEIGHT = 8;
    const int WIDTH = 12;
    std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    for (auto y = 0; y < 4; ++y) {                       // top half
        std::fill_n(data.begin() + y * WIDTH, 6, 20000); // left half
    }
    for (auto y = 4; y < 8; ++y) {                           // bottom half
        std::fill_n(data.begin() + y * WIDTH + 6, 6, 45000); // right half
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
        rerun::DepthImage(rerun::TensorData({HEIGHT, WIDTH}, std::move(data))).with_meter(10000.0)
    );
}
