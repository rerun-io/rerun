#include "mat3x3.hpp"
#include "vec3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct Mat3x3Ext {
            float coeffs[9];

#define Mat3x3 Mat3x3Ext
            // [CODEGEN COPY TO HEADER START]

            static const Mat3x3 IDENTITY;

            /// Creates a new 3x3 matrix from 3 *columns* of 3 elements each.
            Mat3x3(const Vec3D (&_columns)[3])
                : coeffs{
                      _columns[0].x(),
                      _columns[0].y(),
                      _columns[0].z(),
                      _columns[1].x(),
                      _columns[1].y(),
                      _columns[1].z(),
                      _columns[2].x(),
                      _columns[2].y(),
                      _columns[2].z(),
                  } {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef Mat3x3
#else
#define Mat3x3Ext Mat3x3
#endif

        const Mat3x3Ext Mat3x3Ext::IDENTITY = Mat3x3Ext({
            {1.0, 0.0, 0.0},
            {0.0, 1.0, 0.0},
            {0.0, 0.0, 1.0},
        });

    } // namespace datatypes
} // namespace rerun
