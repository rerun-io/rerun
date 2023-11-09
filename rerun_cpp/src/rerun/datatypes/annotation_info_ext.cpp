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
            std::optional<datatypes::Rgba32> color;

#define AnnotationInfo AnnotationInfoExt

            // <CODEGEN_COPY_TO_HEADER>

            AnnotationInfo(
                uint16_t _id, std::optional<std::string> _label = std::nullopt,
                std::optional<datatypes::Rgba32> _color = std::nullopt
            )
                : id(_id), label(std::move(_label)), color(_color) {}

            AnnotationInfo(uint16_t _id, datatypes::Rgba32 _color)
                : id(_id), label(std::nullopt), color(_color) {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

    } // namespace datatypes
} // namespace rerun
