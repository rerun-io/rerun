#include "uvec2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct UVec2DExt {
            uint32_t xy[2];
#define UVec2D UVec2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct UVec2D from x/y values.
            UVec2D(uint32_t x, uint32_t y) : xy{x, y} {}

            /// Construct UVec2D from x/y uint32_t pointer.
            explicit UVec2D(const uint32_t* xy_) : xy{xy_[0], xy_[1]} {}

            uint32_t x() const {
                return xy[0];
            }

            uint32_t y() const {
                return xy[1];
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
