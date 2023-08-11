#include "vector3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Vector3DExt {
            float vector[3];
#define Vector3D Vector3DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Vector3D from x/y/z values.
            Vector3D(float x, float y, float z) : vector{x, y, z} {}

            float x() const {
                return vector.x();
            }

            float y() const {
                return vector.y();
            }

            float z() const {
                return vector.z();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
