#include "vec3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Vec3DExt {
            float xyz[3];
#define Vec3D Vec3DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Vec3D from x/y/z values.
            Vec3D(float x, float y, float z) : xyz{x, y, z} {}

            /// Construct Vec3D from x/y/z float pointer.
            explicit Vec3D(const float* xyz_) : xyz{xyz_[0], xyz_[1], xyz_[2]} {}

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
    } // namespace datatypes
} // namespace rerun
