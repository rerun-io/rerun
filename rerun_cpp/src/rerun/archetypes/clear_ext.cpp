#include "clear.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ClearExt {
            rerun::components::ClearIsRecursive clear;

            // <CODEGEN_COPY_TO_HEADER>

            RERUN_SDK_EXPORT static const Clear FLAT;

            RERUN_SDK_EXPORT static const Clear RECURSIVE;

            Clear(bool _is_recursive = false)
                : Clear(components::ClearIsRecursive(_is_recursive)) {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

        const Clear Clear::FLAT = Clear(false);

        const Clear Clear::RECURSIVE = Clear(true);
    } // namespace archetypes
} // namespace rerun
