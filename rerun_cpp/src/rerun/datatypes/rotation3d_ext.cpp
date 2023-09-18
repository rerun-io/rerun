#include "rotation3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Rotation3DExt {
#define Rotation3D Rotation3DExt

            // [CODEGEN COPY TO HEADER START]

            // static const Rotation3D IDENTITY;

            // [CODEGEN COPY TO HEADER END]
        };

#undef Rotation3DExt
#else
#define Rotation3DExt Rotation3D
#endif

        // TODO(andreas): This constant initialization does not work for unknown reasons!
        //                On clang(-mac) this set the Rotation3D::IDENTITY to all zero instead of
        //                the expected quaternion.
        // Using the same code as a non-constant works fine.
        // const Rotation3DExt Rotation3DExt::IDENTITY = Quaternion::IDENTITY;
    } // namespace datatypes
} // namespace rerun
