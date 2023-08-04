#include "vec3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Vec3DExt {
            float xyz[3];
#define Vec3D Vec3DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Vec3D from x/y/z values.
            Vec3D(float x, float y, float z) : xyz{x, y, z} {}

            float x() const {
                return xyz[0];
            }

            float y() const {
                return xyz[1];
            }

            float z() const {
                return xyz[2];
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
