#include "view_coordinates2d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
//#define EDIT_EXTENSION

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    struct ViewCoordinates2DExt {
        uint8_t coordinates[2];
#define ViewCoordinates2D ViewCoordinates2DExt

        // <CODEGEN_COPY_TO_HEADER>

        /// Construct ViewCoordinates2D from x/y values.
        ViewCoordinates2D(uint8_t axis0, uint8_t axis1)
            : ViewCoordinates2D(rerun::components::ViewCoordinates2D(axis0, axis1)) {}

        /// X=Right, Y=Down (default, image/screen convention).
        RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates2D RD;

        /// X=Right, Y=Up (math/plot convention).
        RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates2D RU;

        /// X=Left, Y=Down (horizontally mirrored image).
        RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates2D LD;

        /// X=Left, Y=Up (both axes flipped).
        RERUN_SDK_EXPORT static const rerun::archetypes::ViewCoordinates2D LU;

        // </CODEGEN_COPY_TO_HEADER>
    };
#endif

    const ViewCoordinates2D ViewCoordinates2D::RD =
        ViewCoordinates2D(rerun::components::ViewCoordinates2D::RD);
    const ViewCoordinates2D ViewCoordinates2D::RU =
        ViewCoordinates2D(rerun::components::ViewCoordinates2D::RU);
    const ViewCoordinates2D ViewCoordinates2D::LD =
        ViewCoordinates2D(rerun::components::ViewCoordinates2D::LD);
    const ViewCoordinates2D ViewCoordinates2D::LU =
        ViewCoordinates2D(rerun::components::ViewCoordinates2D::LU);

} // namespace rerun::archetypes
