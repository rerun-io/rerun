#include "clear.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ClearExt {
            rerun::components::ClearIsRecursive clear;

            // [CODEGEN COPY TO HEADER START]

            static Clear flat() {
                return Clear(false);
            }

            static Clear recursive() {
                return Clear(true);
            }

            Clear(bool recursive = false) : Clear(components::ClearIsRecursive(recursive)) {}

            // [CODEGEN COPY TO HEADER END]
        };

#endif

    } // namespace archetypes
} // namespace rerun
