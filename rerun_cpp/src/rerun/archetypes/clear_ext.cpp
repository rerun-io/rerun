#include "clear.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ClearExt {
            rerun::components::ClearIsRecursive clear;

            // [CODEGEN COPY TO HEADER START]

            static const Clear FLAT;

            static const Clear RECURSIVE;

            Clear(bool is_recursive = false) : Clear(components::ClearIsRecursive(is_recursive)) {}

            // [CODEGEN COPY TO HEADER END]
        };

        const Clear Clear::FLAT = Clear(false);

        const Clear Clear::RECURSIVE = Clear(true);
#endif

    } // namespace archetypes
} // namespace rerun
