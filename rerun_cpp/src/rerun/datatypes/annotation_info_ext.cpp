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

            AnnotationInfo(std::pair<uint16_t, std::string> id_and_label)
                : id(id_and_label.first),
                  label(std::move(id_and_label.second)),
                  color(std::nullopt) {}

            AnnotationInfo(std::pair<uint16_t, components::Color> id_and_color)
                : id(id_and_color.first),
                  label(std::nullopt),
                  color(std::move(id_and_color.second)) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif

    } // namespace datatypes
} // namespace rerun
