#include <utility>
#include "annotation_context.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {
#ifdef EDIT_EXTENSION
        struct AnnotationContextExt {
            std::vector<rerun::datatypes::ClassDescriptionMapElem> class_map;

#define AnnotationContext AnnotationContextExt

            // <CODEGEN_COPY_TO_HEADER>

            AnnotationContext(
                std::initializer_list<rerun::datatypes::ClassDescription> class_descriptions
            ) {
                class_map.reserve(class_descriptions.size());
                for (const auto& class_description : class_descriptions) {
                    class_map.emplace_back(std::move(class_description));
                }
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

    } // namespace components
} // namespace rerun
