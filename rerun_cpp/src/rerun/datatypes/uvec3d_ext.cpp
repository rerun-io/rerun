#include "uvec3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct UVec3DExt {
            uint32_t xyz[3];
#define UVec3D UVec3DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct UVec3D from x/y/z values.
            UVec3D(uint32_t x, uint32_t y, uint32_t z) : xyz{x, y, z} {}

            /// Construct UVec3D from x/y/z uint32_t pointer.
            explicit UVec3D(const uint32_t* xyz_) : xyz{xyz_[0], xyz_[1], xyz_[2]} {}

            uint32_t x() const {
                return xyz[0];
            }

            uint32_t y() const {
                return xyz[1];
            }

            uint32_t z() const {
                return xyz[2];
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
