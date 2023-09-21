#include "material.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct MaterialExt {
#define Material MaterialExt

            // [CODEGEN COPY TO HEADER START]

            static Material from_albedo_factor(rerun::datatypes::Color color) {
                return Material(color);
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
