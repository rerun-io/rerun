// Log a segmentation image with annotations.

#include <rerun.hpp>

#include <algorithm> // fill_n
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_annotation_context_segmentation");
    rec.spawn().exit_on_failure();

    // create an annotation context to describe the classes
    rec.log_static(
        "segmentation",
        rerun::AnnotationContext({
            rerun::AnnotationInfo(1, "red", rerun::Rgba32(255, 0, 0)),
            rerun::AnnotationInfo(2, "green", rerun::Rgba32(0, 255, 0)),
        })
    );

    // create a segmentation image
    const int HEIGHT = 200;
    const int WIDTH = 300;
    std::vector<uint8_t> data(WIDTH * HEIGHT, 0);
    for (auto y = 50; y < 100; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 50, 70, static_cast<uint8_t>(1));
    }
    for (auto y = 100; y < 180; ++y) {
        std::fill_n(data.begin() + y * WIDTH + 130, 150, static_cast<uint8_t>(2));
    }

    rec.log("segmentation/image", rerun::SegmentationImage(data.data(), {WIDTH, HEIGHT}));
}
