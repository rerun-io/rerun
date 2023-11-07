#include "rotation_axis_angle.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct RotationAxisAngleExt {
            Vec3D axis;
            Angle angle;

#define RotationAxisAngle RotationAxisAngleExt

            // <CODEGEN_COPY_TO_HEADER>

            RotationAxisAngle(const Vec3D& _axis, const Angle& _angle)
                : axis(_axis), angle(_angle) {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

    } // namespace datatypes
} // namespace rerun
