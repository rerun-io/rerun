#include "resolution2d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Resolution2DExt {
            rerun::datatypes::Vec2D resolution;
#define Resolution2D Resolution2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct resolution from width and height.
            Resolution2D(uint32_t width, uint32_t height) : wh{width, height} {}

            uint32_t width() const {
                return wh.x();
            }

            uint32_t height() const {
                return wh.y();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef Resolution2DExt
#endif
    } // namespace components
} // namespace rerun
