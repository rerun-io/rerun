// Create and log a segmentation image.

#include <rerun.hpp>

#include <algorithm> // std::fill_n
#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_annotation_context_connections");
    rec.spawn().exit_on_failure();

    // Create a segmentation image
    const int HEIGHT = 8;
    const int WIDTH = 12;
    std::vector<uint8_t> data(WIDTH * HEIGHT, 0);
    for (auto y = 0; y < 4; ++y) {                                         // top half
        std::fill_n(data.begin() + y * WIDTH, 6, static_cast<uint8_t>(1)); // left half
    }
    for (auto y = 4; y < 8; ++y) {                                             // bottom half
        std::fill_n(data.begin() + y * WIDTH + 6, 6, static_cast<uint8_t>(2)); // right half
    }

    // create an annotation context to describe the classes
    rec.log_timeless(
        "/",
        rerun::AnnotationContext({
            rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
            rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
        })
    );

    rec.log("image", rerun::SegmentationImage({HEIGHT, WIDTH}, data));
}
