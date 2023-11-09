#include "half_sizes3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct HalfSizes3DExt {
            float xyz[3];
#define HalfSizes3D HalfSizes3DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct HalfSizes3D from x/y/z values.
            HalfSizes3D(float x, float y, float z) : xyz{x, y, z} {}

            float x() const {
                return xyz.x();
            }

            float y() const {
                return xyz.y();
            }

            float z() const {
                return xyz.z();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
