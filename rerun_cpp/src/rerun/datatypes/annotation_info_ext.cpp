#include <utility>
#include "annotation_info.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct AnnotationInfoExt {
            uint16_t id;
            std::optional<datatypes::Utf8> label;
            std::optional<datatypes::Color> color;

#define AnnotationInfo AnnotationInfoExt

            // [CODEGEN COPY TO HEADER START]

            AnnotationInfo(
                uint16_t _id, std::optional<std::string> _label = std::nullopt,
                std::optional<datatypes::Color> _color = std::nullopt
            )
                : id(_id), label(std::move(_label)), color(_color) {}

            AnnotationInfo(uint16_t _id, datatypes::Color _color)
                : id(_id), label(std::nullopt), color(_color) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif

    } // namespace datatypes
} // namespace rerun
