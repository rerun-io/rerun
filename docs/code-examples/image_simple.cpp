// Create and log a image.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_image_simple");
    rec.connect().throw_on_failure();

    // Create a synthetic image.
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint8_t> data(WIDTH * HEIGHT * 3, 0);
    for (size_t i = 0; i < data.size(); i += 3) {
        data[i] = 255;
    }
    for (auto y = 50; y < 150; ++y) { // top half
        auto row = data.begin() + y * WIDTH * 3;
        for (auto i = 50; i < 150 * 3; i += 3) { // left half
            row[i] = 0;
            row[i + 1] = 255;
        }
    }

    rec.log("image", rerun::Image({HEIGHT, WIDTH, 3}, std::move(data)));
}
