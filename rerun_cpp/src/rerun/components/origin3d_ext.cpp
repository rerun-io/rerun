#include "origin3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Origin3DExt {
            float origin[3];
#define Origin3D Origin3DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Origin3D from x/y/z values.
            Origin3D(float x, float y, float z) : origin{x, y, z} {}

            float x() const {
                return origin.x();
            }

            float y() const {
                return origin.y();
            }

            float z() const {
                return origin.z();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
