#include "class_description.hpp"

namespace rerun::datatypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Create a new `ClassDescription` from a single annotation info.
    ClassDescription(
        uint16_t id, std::optional<std::string> label = std::nullopt,
        std::optional<datatypes::Rgba32> color = std::nullopt
    )
        : info(id, label, color) {}

    ClassDescription(
        AnnotationInfo info_, Collection<AnnotationInfo> keypoint_annotations_ = {},
        Collection<KeypointPair> keypoint_connections_ = {}
    )
        : info(std::move(info_)),
            keypoint_annotations(std::move(keypoint_annotations_)),
            keypoint_connections(std::move(keypoint_connections_)) {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::datatypes
