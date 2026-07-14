// Create and log a image.

#include <rerun.hpp>

#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_image");
    rec.spawn().exit_on_failure();

    // Create a synthetic image.
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint8_t> data(WIDTH * HEIGHT * 3, 0);
    for (size_t i = 0; i < data.size(); i += 3) {
        data[i] = 255;
    }
    for (size_t y = 50; y < 150; ++y) {
        for (size_t x = 50; x < 150; ++x) {
            data[(y * WIDTH + x) * 3 + 0] = 0;
            data[(y * WIDTH + x) * 3 + 1] = 255;
            data[(y * WIDTH + x) * 3 + 2] = 0;
        }
    }

    rec.log("image", rerun::Image::from_rgb24(data, {WIDTH, HEIGHT}));
}
