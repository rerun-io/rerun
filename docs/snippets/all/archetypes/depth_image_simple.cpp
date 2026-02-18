// Create and log a depth image.

#include <rerun.hpp>

#include <algorithm> // fill_n
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_depth_image_simple");
    rec.spawn().exit_on_failure();

    // create a synthetic depth image.
    const uint32_t HEIGHT = 200;
    const uint32_t WIDTH = 300;
    std::vector<uint16_t> pixels(WIDTH * HEIGHT, 65535);
    for (uint32_t y = 50; y < 150; ++y) {
        std::fill_n(pixels.begin() + y * WIDTH + 50, 100, static_cast<uint16_t>(20000));
    }
    for (uint32_t y = 130; y < 180; ++y) {
        std::fill_n(pixels.begin() + y * WIDTH + 100, 180, static_cast<uint16_t>(45000));
    }

    rec.log("depth", rerun::DepthImage(pixels.data(), {WIDTH, HEIGHT}).with_meter(10000.0));
}
