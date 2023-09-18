// Log an annotation context to assign a label and color to each class

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_annotation_context_rects");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // Log an annotation context to assign a label and color to each class
    rec.log(
        "/",
        rr::AnnotationContext({
            rr::datatypes::AnnotationInfo(1, "red", rr::datatypes::Color(255, 0, 0)),
            rr::datatypes::AnnotationInfo(2, "green", rr::datatypes::Color(0, 255, 0)),
        })
    );

    // Log a batch of 2 rectangles with different class IDs
    rec.log(
        "detections",
        rr::Boxes2D::from_mins_and_sizes(
            {{-2.0f, -2.0f}, {0.0f, 0.f}},
            {{3.0f, 3.0f}, {2.0f, 2.0f}}
        ).with_class_ids({1, 2})
    );

    // Log an extra rect to set the view bounds
    rec.log("bounds", rr::Boxes2D::from_mins_and_sizes({{0.0f, 0.0f}}, {{5.0f, 5.0f}}));
}
