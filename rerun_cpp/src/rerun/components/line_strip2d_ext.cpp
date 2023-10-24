#include "line_strip2d.hpp"

#include <algorithm>

// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        template <typename T>
        LineStrip2D(const std::vector<T>& points_) : points(points_.size()) {
            std::transform(points_.begin(), points_.end(), points.begin(), [](const T& pt) {
                return rerun::datatypes::Vec2D(pt);
            });
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace components
} // namespace rerun
