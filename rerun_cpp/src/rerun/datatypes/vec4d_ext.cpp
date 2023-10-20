#include "vec4d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Vec4DExt {
            float xyzw[4];
#define Vec4D Vec4DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Vec4D from x/y/z/w values.
            Vec4D(float x, float y, float z, float w) : xyzw{x, y, z, w} {}

            /// Construct Vec4D from x/y/z/w float pointer.
            ///
            /// Attention: The pointer must point to at least least 4 floats long.
            Vec4D(const float* ptr) : xyzw{ptr[0], ptr[1], ptr[2], ptr[3]} {}

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
    } // namespace datatypes
} // namespace rerun
