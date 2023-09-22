#include "view_coordinates.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct ViewCoordinatesExt {
            uint8_t coordinates[3];
#define ViewCoordinates ViewCoordinatesExt

            // [CODEGEN COPY TO HEADER START]

            enum ViewDir : uint8_t {
                Up = 1,
                Down = 2,
                Right = 3,
                Left = 4,
                Forward = 5,
                Back = 6,
            };

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : coordinates{axis0, axis1, axis2} {}

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(ViewDir axis0, ViewDir axis1, ViewDir axis2)
                : coordinates{axis0, axis1, axis2} {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace datatypes
} // namespace rerun
