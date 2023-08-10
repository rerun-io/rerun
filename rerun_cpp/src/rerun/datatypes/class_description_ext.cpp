#include "class_description.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ClassDescriptionExt {
            AnnotationInfo info;
            std::vector<AnnotationInfo> keypoint_annotations;
            std::vector<KeypointPair> keypoint_connections;

#define ClassDescription ClassDescriptionExt

            // [CODEGEN COPY TO HEADER START]

            ClassDescription(
                AnnotationInfo _info, std::vector<AnnotationInfo> _keypoint_annotations = {},
                std::vector<KeypointPair> _keypoint_connections = {}
            )
                : info(std::move(_info)),
                  keypoint_annotations(std::move(_keypoint_annotations)),
                  keypoint_connections(std::move(_keypoint_connections)) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif

    } // namespace datatypes
} // namespace rerun
