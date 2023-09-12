#include "clear.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ClearExt {
            rerun::components::ClearSettings clear;

#define Clear ClearExt

            // [CODEGEN COPY TO HEADER START]

            static Clear flat() {
                return Clear(false);
            }

            static Clear recursive() {
                return Clear(true);
            }

            Clear(bool recursive = false) : Clear(components::ClearSettings(recursive)) {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef ClearExt
#else
#define ClearExt Clear
#endif

    } // namespace archetypes
} // namespace rerun
