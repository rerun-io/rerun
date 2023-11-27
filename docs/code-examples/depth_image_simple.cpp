// Create and log a depth image.

#include <rerun.hpp>

#include <algorithm> // fill_n
#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_depth_image");
    rec.spawn().exit_on_failure();

    // create a synthetic depth image.
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint16_t> data(WIDTH * HEIGHT, 65535);
    for (auto y = 50; y < 150; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 50, 100, static_cast<uint16_t>(20000));
    }
    for (auto y = 130; y < 180; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 100, 180, static_cast<uint16_t>(45000));
    }

    rec.log("depth", rerun::DepthImage({HEIGHT, WIDTH}, data.data()).with_meter(10000.0));
}
