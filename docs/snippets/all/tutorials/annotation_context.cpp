#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_annotation_context_connections");
    rec.spawn().exit_on_failure();

    // Annotation context with two classes, using two labeled classes, of which ones defines a
    // color.
    rec.log_static(
        "masks",
        rerun::AnnotationContext({
            rerun::AnnotationInfo(0, "Background"),
            rerun::AnnotationInfo(1, "Person", rerun::Rgba32(255, 0, 0)),
        })
    );

    // Annotation context with simple keypoints & keypoint connections.
    std::vector<rerun::AnnotationInfo> keypoint_annotations;
    for (uint16_t i = 0; i < 10; ++i) {
        keypoint_annotations.push_back(
            rerun::AnnotationInfo(i, rerun::Rgba32(0, static_cast<uint8_t>(28 * i), 0))
        );
    }

    std::vector<rerun::KeypointPair> keypoint_connections;
    for (uint16_t i = 0; i < 9; ++i) {
        keypoint_connections.push_back(rerun::KeypointPair(i, i + 1));
    }

    rec.log_static(
        "detections", // Applies to all entities below "detections".
        rerun::AnnotationContext({rerun::ClassDescription(
            rerun::AnnotationInfo(0, "Snake"),
            keypoint_annotations,
            keypoint_connections
        )})
    );
}
