#include <utility>
#include "class_description_map_elem.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ClassDescriptionMapElemExt {
            components::ClassId class_id;
            ClassDescription class_description;

#define ClassDescriptionMapElem ClassDescriptionMapElemExt

            // <CODEGEN_COPY_TO_HEADER>

            ClassDescriptionMapElem(ClassDescription _class_description)
                : class_id(_class_description.info.id),
                  class_description(std::move(_class_description)) {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

    } // namespace datatypes
} // namespace rerun
