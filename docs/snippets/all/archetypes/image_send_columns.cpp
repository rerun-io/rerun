// Send multiple images at once using `send_columns`.

#include <numeric>
#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_image_send_columns");
    rec.spawn().exit_on_failure();

    // Timeline on which the images are distributed.
    std::vector<int64_t> times(20);
    std::iota(times.begin(), times.end(), 0);

    // Create a batch of images with a moving rectangle.
    const size_t width = 300, height = 200;
    std::vector<uint8_t> images(times.size() * height * width * 3, 0);
    for (size_t t = 0; t < times.size(); ++t) {
        for (size_t y = 0; y < height; ++y) {
            for (size_t x = 0; x < width; ++x) {
                size_t idx = (t * height * width + y * width + x) * 3;
                images[idx + 2] = 255; // Blue background
                if (y >= 50 && y < 150 && x >= t * 10 && x < t * 10 + 100) {
                    images[idx + 1] = 255; // Turquoise rectangle
                }
            }
        }
    }

    // Log the ImageFormat and indicator once, as static.
    auto format = rerun::components::ImageFormat(
        {width, height},
        rerun::ColorModel::RGB,
        rerun::ChannelDatatype::U8
    );
    rec.log_static("images", rerun::borrow(&format), rerun::Image::IndicatorComponent());

    // Split up the image data into several components referencing the underlying data.
    const size_t image_size_in_bytes = width * height * 3;
    std::vector<rerun::components::ImageBuffer> image_data(times.size());
    for (size_t i = 0; i < times.size(); ++i) {
        image_data[i] = rerun::borrow(images.data() + i * image_size_in_bytes, image_size_in_bytes);
    }

    // Send all images at once.
    rec.send_columns(
        "images",
        rerun::TimeColumn::from_sequence_points("step", std::move(times)),
        rerun::Image().with_many_buffer(std::move(image_data)).columns()
    );
}
