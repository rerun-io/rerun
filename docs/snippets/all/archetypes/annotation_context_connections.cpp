// Log annotation context with connections between keypoints.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_annotation_context_connections");
    rec.spawn().exit_on_failure();

    // Log an annotation context to assign a label and color to each class
    // Create a class description with labels and color for each keypoint ID as well as some
    // connections between keypoints.
    rec.log_static(
        "/",
        rerun::AnnotationContext({rerun::ClassDescription{
            0,
            {
                rerun::AnnotationInfo(0, "zero", rerun::Rgba32(255, 0, 0)),
                rerun::AnnotationInfo(1, "one", rerun::Rgba32(0, 255, 0)),
                rerun::AnnotationInfo(2, "two", rerun::Rgba32(0, 0, 255)),
                rerun::AnnotationInfo(3, "three", rerun::Rgba32(255, 255, 0)),
            },
            {{0, 2}, {1, 2}, {2, 3}},
        }})
    );

    // Log some points with different keypoint IDs
    rec.log(
        "points",
        rerun::Points3D({{0.0f, 0.0f, 0.0f},
                         {50.0f, 0.0f, 20.0f},
                         {100.0f, 100.0f, 30.0f},
                         {0.0f, 50.0f, 40.0f}})
            .with_keypoint_ids({0, 1, 2, 3})
            .with_class_ids({0})
    );
}
