#include "quaternion.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct QuaternionExt {
            float xyzw[4];
#define Quaternion QuaternionExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Quaternion from x/y/z/w values.
            Quaternion(float x, float y, float z, float w) : xyzw{x, y, z, w} {}

            float x() const {
                return xyzw[0];
            }

            float y() const {
                return xyzw[1];
            }

            float z() const {
                return xyzw[2];
            }

            float w() const {
                return xyzw[3];
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
