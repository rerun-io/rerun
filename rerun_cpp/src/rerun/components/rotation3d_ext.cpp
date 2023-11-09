#include "rotation3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Rotation3DExt {
            Rotation3D repr;
#define Rotation3D Rotation3DExt

            // <CODEGEN_COPY_TO_HEADER>

            static const Rotation3D IDENTITY;

            /// Construct Rotation3d from Quaternion.
            Rotation3D(datatypes::Quaternion quaternion) : repr{quaternion} {}

            /// Construct Rotation3d from axis-angle
            Rotation3D(datatypes::RotationAxisAngle axis_angle) : repr{axis_angle} {}

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef Rotation3DExt
#else
#define Rotation3DExt Rotation3D
#endif

        const Rotation3DExt Rotation3DExt::IDENTITY = datatypes::Quaternion::IDENTITY;
    } // namespace components
} // namespace rerun
