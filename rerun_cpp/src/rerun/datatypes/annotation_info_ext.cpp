#include <utility>
#include "annotation_info.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct AnnotationInfoExt {
            uint16_t id;
            std::optional<components::Label> label;
            std::optional<components::Color> color;

#define AnnotationInfo AnnotationInfoExt

            // [CODEGEN COPY TO HEADER START]

            AnnotationInfo(
                uint16_t _id, std::optional<std::string> _label = std::nullopt,
                std::optional<components::Color> _color = std::nullopt
            )
                : id(_id), label(std::move(_label)), color(_color) {}

            AnnotationInfo(uint16_t _id, components::Color _color)
                : id(_id), label(std::nullopt), color(_color) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif

    } // namespace datatypes
} // namespace rerun
