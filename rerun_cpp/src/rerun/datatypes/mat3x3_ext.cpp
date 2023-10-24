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
            Mat3x3(const Vec3D (&columns)[3])
                : flat_columns{
                      columns[0].x(),
                      columns[0].y(),
                      columns[0].z(),
                      columns[1].x(),
                      columns[1].y(),
                      columns[1].z(),
                      columns[2].x(),
                      columns[2].y(),
                      columns[2].z(),
                  } {}

            /// Construct a new 3x3 matrix from a pointer to 9 floats (in row major order).
            explicit Mat3x3(const float* elements)
                : flat_columns{
                      elements[0],
                      elements[1],
                      elements[2],
                      elements[3],
                      elements[4],
                      elements[5],
                      elements[6],
                      elements[7],
                      elements[8],
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
