#include "mat4x4.hpp"
#include "vec4d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Mat4x4Ext {
            float coeffs[9];

#define Mat4x4 Mat4x4Ext
            // [CODEGEN COPY TO HEADER START]

            static const Mat4x4 IDENTITY;

            /// Creates a new 4x4 matrix from 3 *columns* of 4 elements each.
            Mat4x4(const Vec4D (&_columns)[4])
                : coeffs{
                      _columns[0].x(),
                      _columns[0].y(),
                      _columns[0].z(),
                      _columns[0].w(),
                      _columns[1].x(),
                      _columns[1].y(),
                      _columns[1].z(),
                      _columns[1].w(),
                      _columns[2].x(),
                      _columns[2].y(),
                      _columns[2].z(),
                      _columns[2].w(),
                      _columns[3].x(),
                      _columns[3].y(),
                      _columns[3].z(),
                      _columns[3].w(),
                  } {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef Mat4x4
#else
#define Mat4x4Ext Mat4x4
#endif

        const Mat4x4Ext Mat4x4Ext::IDENTITY = Mat4x4Ext({
            {1.0f, 0.0f, 0.0f, 0.0f},
            {0.0f, 1.0f, 0.0f, 0.0f},
            {0.0f, 0.0f, 1.0f, 0.0f},
            {0.0f, 0.0f, 0.0f, 1.0f},
        });

    } // namespace datatypes
} // namespace rerun
