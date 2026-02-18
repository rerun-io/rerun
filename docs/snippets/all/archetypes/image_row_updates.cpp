//! Update an image over time.
//!
//! See also the `image_column_updates` example, which achieves the same thing in a single operation.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    auto rec = rerun::RecordingStream("rerun_example_image_row_updates");
    rec.spawn().exit_on_failure();

    const size_t HEIGHT = 200;
    const size_t WIDTH = 300;

    for (size_t t = 0; t < 20; ++t) {
        rec.set_time_sequence("time", static_cast<int64_t>(t));

        std::vector<uint8_t> data(WIDTH * HEIGHT * 3, 0);
        for (size_t i = 0; i < data.size(); i += 3) {
            data[i + 2] = 255;
        }
        for (size_t y = 50; y < 150; ++y) {
            for (size_t x = t * 10; x < t * 10 + 100; ++x) {
                data[(y * WIDTH + x) * 3 + 0] = 0;
                data[(y * WIDTH + x) * 3 + 1] = 255;
                data[(y * WIDTH + x) * 3 + 2] = 255;
            }
        }

        rec.log("image", rerun::Image::from_rgb24(data, {WIDTH, HEIGHT}));
    }
}
