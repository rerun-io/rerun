#include "boxes2d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Creates new `Boxes2D` with `half_sizes` and `centers` created from minimums and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        static Boxes2D from_mins_and_sizes(
            const std::vector<datatypes::Vec2D>& xy, const std::vector<datatypes::Vec2D>& extents
        );

        // [CODEGEN COPY TO HEADER END]
#endif

        Boxes2D Boxes2D::from_mins_and_sizes(
            const std::vector<datatypes::Vec2D>& xy, const std::vector<datatypes::Vec2D>& extents
        ) {
            std::vector<components::HalfExtents2D> half_extents;
            half_extents.reserve(extents.size());
            for (const auto& wh : extents) {
                half_extents.emplace_back(wh.x() / 2.0, wh.y() / 2.0);
            }

            auto num_centers = std::min(xy.size(), half_extents.size());
            std::vector<components::Origin2D> centers;
            centers.reserve(num_centers);
            for (size_t i = 0; i < num_centers; ++i) {
                centers.emplace_back(
                    xy[i].x() + half_extents[i].x(),
                    xy[i].y() + half_extents[i].y()
                );
            }

            return Boxes2D(half_extents).with_centers(centers);
        }

    } // namespace archetypes
} // namespace rerun
