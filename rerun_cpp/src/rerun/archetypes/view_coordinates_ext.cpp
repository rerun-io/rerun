#include "view_coordinates.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct ViewCoordinatesExt {
            uint8_t coordinates[3];
#define ViewCoordinates ViewCoordinatesExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Vec3D from x/y/z values.
            constexpr ViewCoordinates(uint8_t axis0, uint8_t axis1, uint8_t axis2)
                : coordinates(rerun::components::ViewCoordinates(axis0, axis1, axis2)) {}

            static const rerun::archetypes::ViewCoordinates RDF;

            // [CODEGEN COPY TO HEADER END]
        };
#endif

        const ViewCoordinates ViewCoordinates::RDF = ViewCoordinates(
            rerun::components::ViewCoordinates::Right, rerun::components::ViewCoordinates::Down,
            rerun::components::ViewCoordinates::Forward
        );

    } // namespace archetypes
} // namespace rerun
