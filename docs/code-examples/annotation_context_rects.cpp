// Log an annotation context to assign a label and color to each class

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_annotation_context_rects");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // Log an annotation context to assign a label and color to each class
    rec.log_timeless(
        "/",
        rerun::AnnotationContext({
            rerun::datatypes::AnnotationInfo(1, "red", rerun::datatypes::Rgba32(255, 0, 0)),
            rerun::datatypes::AnnotationInfo(2, "green", rerun::datatypes::Rgba32(0, 255, 0)),
        })
    );

    // Log a batch of 2 rectangles with different class IDs
    rec.log(
        "detections",
        rerun::Boxes2D::from_mins_and_sizes(
            {{-2.0f, -2.0f}, {0.0f, 0.f}},
            {{3.0f, 3.0f}, {2.0f, 2.0f}}
        ).with_class_ids({1, 2})
    );

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_half_sizes({{2.5f, 2.5f}}));
}
