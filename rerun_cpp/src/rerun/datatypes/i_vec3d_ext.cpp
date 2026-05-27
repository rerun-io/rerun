#include "i_vec3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct IVec3DExt {
            int32_t xyz[3];
#define IVec3D IVec3DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct IVec3D from x/y/z values.
            IVec3D(int32_t x, int32_t y, int32_t z) : xyz{x, y, z} {}

            /// Construct IVec3D from x/y/z int32_t pointer.
            explicit IVec3D(const int32_t* xyz_) : xyz{xyz_[0], xyz_[1], xyz_[2]} {}

            int32_t x() const {
                return xyz[0];
            }

            int32_t y() const {
                return xyz[1];
            }

            int32_t z() const {
                return xyz[2];
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
