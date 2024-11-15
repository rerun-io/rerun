#include "view_coordinates.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ViewCoordinatesExt {
            uint8_t coordinates[3];
#define ViewCoordinates ViewCoordinatesExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct Vec3D from x/y/z values.
            explicit constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : coordinates{axis0, axis1, axis2} {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace datatypes
} // namespace rerun
