#include "mat3x3.hpp"

// [CODEGEN COPY TO HEADER START]
#include "vec3d.hpp"

// [CODEGEN COPY TO HEADER END]

namespace rerun {
    namespace datatypes {

#if 0
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
#endif

        const Mat3x3 Mat3x3::IDENTITY = Mat3x3({
            {1.0, 0.0, 0.0},
            {0.0, 1.0, 0.0},
            {0.0, 0.0, 1.0},
        });

    } // namespace datatypes
} // namespace rerun
