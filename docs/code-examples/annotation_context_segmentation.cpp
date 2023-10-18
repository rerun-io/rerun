// Log a segmentation image with annotations.

#include <rerun.hpp>

#include <algorithm>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_annotation_context_connections");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // create an annotation context to describe the classes
    rec.log_timeless(
        "segmentation",
        rerun::AnnotationContext({
            rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
            rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
        })
    );

    // create a segmentation image
    const int HEIGHT = 8;
    const int WIDTH = 12;
    std::vector<uint8_t> data(WIDTH * HEIGHT, 0);
    for (auto y = 0; y < 4; ++y) {                   // top half
        std::fill_n(data.begin() + y * WIDTH, 6, 1); // left half
    }
    for (auto y = 4; y < 8; ++y) {                       // bottom half
        std::fill_n(data.begin() + y * WIDTH + 6, 6, 2); // right half
    }

    rec.log("segmentation/image", rerun::SegmentationImage(WIDTH, HEIGHT, std::move(data)));
}
