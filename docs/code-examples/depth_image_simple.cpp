// Create and log a depth image.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_depth_image");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // create a synthetic depth image.
    const int HEIGHT = 8;
    const int WIDTH = 12;
    std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    for (auto y = 0; y < 4; ++y) { // top half
        auto row = data.begin() + y * WIDTH;
        std::fill(row, row + 6, 20000); // left half
    }
    for (auto y = 4; y < 8; ++y) { // bottom half
        auto row = data.begin() + y * WIDTH;
        std::fill(row + 6, row + 12, 45000); // right half
    }

    rec.log(
        "depth",
        rerun::DepthImage(rerun::TensorData({HEIGHT, WIDTH}, std::move(data))).with_meter(10000.0)
    );
}
