#include "rotation3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Rotation3DExt {
#define Rotation3D Rotation3DExt

            // <CODEGEN_COPY_TO_HEADER>

            RERUN_SDK_EXPORT static const Rotation3D IDENTITY;

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef Rotation3DExt
#else
#define Rotation3DExt Rotation3D
#endif

        const Rotation3DExt Rotation3DExt::IDENTITY = Quaternion::IDENTITY;
    } // namespace datatypes
} // namespace rerun
