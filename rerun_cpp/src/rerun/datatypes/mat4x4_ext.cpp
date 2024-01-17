#include "mat4x4.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "vec4d.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun {
    namespace datatypes {

#if 0
        // <CODEGEN_COPY_TO_HEADER>

        static const Mat4x4 IDENTITY;

        /// Creates a new 4x4 matrix from 3 *columns* of 4 elements each.
        Mat4x4(const Vec4D (&columns)[4])
            : flat_columns{
                  columns[0].x(),
                  columns[0].y(),
                  columns[0].z(),
                  columns[0].w(),
                  columns[1].x(),
                  columns[1].y(),
                  columns[1].z(),
                  columns[1].w(),
                  columns[2].x(),
                  columns[2].y(),
                  columns[2].z(),
                  columns[2].w(),
                  columns[3].x(),
                  columns[3].y(),
                  columns[3].z(),
                  columns[3].w(),
              } {}

        /// Construct a new 4x4 matrix from a pointer to 16 floats (in column major order).
        explicit Mat4x4(const float* elements)
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
                  elements[9],
                  elements[10],
                  elements[11],
                  elements[12],
                  elements[13],
                  elements[14],
                  elements[15],
              } {}

        // </CODEGEN_COPY_TO_HEADER>
#endif

        const Mat4x4 Mat4x4::IDENTITY = Mat4x4({
            {1.0f, 0.0f, 0.0f, 0.0f},
            {0.0f, 1.0f, 0.0f, 0.0f},
            {0.0f, 0.0f, 1.0f, 0.0f},
            {0.0f, 0.0f, 0.0f, 1.0f},
        });

    } // namespace datatypes
} // namespace rerun
