#include "line_strip3d.hpp"

#include <algorithm>

// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        template <typename T>
        LineStrip3D(const std::vector<T>& points_) : points(points_.size()) {
            std::transform(points_.begin(), points_.end(), points.begin(), [](const T& pt) {
                return rerun::datatypes::Vec3D(pt);
            });
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace components
} // namespace rerun
