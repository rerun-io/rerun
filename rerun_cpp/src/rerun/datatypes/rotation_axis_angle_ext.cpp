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

            // [CODEGEN COPY TO HEADER START]

            RotationAxisAngle(const Vec3D& _axis, const Angle& _angle)
                : axis(_axis), angle(_angle) {}

            // [CODEGEN COPY TO HEADER END]
        };
#endif

    } // namespace datatypes
} // namespace rerun
