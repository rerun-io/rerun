#include "origin2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Origin2DExt {
            float origin[3];
#define Origin2D Origin2DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Origin2D from x/y/z values.
            Origin2D(float x, float y) : origin{x, y} {}

            float x() const {
                return origin.x();
            }

            float y() const {
                return origin.y();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
