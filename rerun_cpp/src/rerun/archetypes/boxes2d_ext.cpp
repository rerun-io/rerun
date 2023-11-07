#include "boxes2d.hpp"

#include "../component_batch_adapter_builtins.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new `Boxes2D` with `half_sizes` centered around the local origin.
        static Boxes2D from_half_sizes(ComponentBatch<components::HalfSizes2D> half_sizes) {
            Boxes2D boxes;
            boxes.half_sizes = std::move(half_sizes);
            return boxes;
        }

        /// Creates new `Boxes2D` with `centers` and `half_sizes`.
        static Boxes2D from_centers_and_half_sizes(
            ComponentBatch<components::Position2D> centers,
            ComponentBatch<components::HalfSizes2D> half_sizes
        ) {
            Boxes2D boxes;
            boxes.half_sizes = std::move(half_sizes);
            boxes.centers = std::move(centers);
            return boxes;
        }

        /// Creates new `Boxes2D` with `half_sizes` created from (full) sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the
        /// input data.
        static Boxes2D from_sizes(const std::vector<datatypes::Vec2D>& sizes);

        /// Creates new `Boxes2D` with `centers` and `half_sizes` created from centers and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        static Boxes2D from_centers_and_sizes(
            ComponentBatch<components::Position2D> centers,
            const std::vector<datatypes::Vec2D>& sizes
        ) {
            Boxes2D boxes = from_sizes(std::move(sizes));
            boxes.centers = std::move(centers);
            return boxes;
        }

        /// Creates new `Boxes2D` with `half_sizes` and `centers` created from minimums and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        static Boxes2D from_mins_and_sizes(
            const std::vector<datatypes::Vec2D>& mins, const std::vector<datatypes::Vec2D>& sizes
        );

        // </CODEGEN_COPY_TO_HEADER>
#endif
        Boxes2D Boxes2D::from_sizes(const std::vector<datatypes::Vec2D>& sizes) {
            std::vector<components::HalfSizes2D> half_sizes;
            half_sizes.reserve(sizes.size());
            for (const auto& size : sizes) {
                half_sizes.emplace_back(size.x() / 2.0f, size.y() / 2.0f);
            }

            // Move the vector into a component batch.
            return Boxes2D::from_half_sizes(std::move(half_sizes));
        }

        Boxes2D Boxes2D::from_mins_and_sizes(
            const std::vector<datatypes::Vec2D>& mins, const std::vector<datatypes::Vec2D>& sizes
        ) {
            auto num_components = std::min(mins.size(), sizes.size());

            std::vector<components::HalfSizes2D> half_sizes;
            std::vector<components::Position2D> centers;
            half_sizes.reserve(num_components);
            centers.reserve(num_components);

            for (size_t i = 0; i < num_components; ++i) {
                float half_size_x = sizes[i].x() * 0.5f;
                float half_size_y = sizes[i].y() * 0.5f;

                half_sizes.emplace_back(half_size_x, half_size_y);
                centers.emplace_back(mins[i].x() + half_size_x, mins[i].y() + half_size_y);
            }

            Boxes2D boxes;
            boxes.half_sizes = std::move(half_sizes);
            boxes.centers = std::move(centers);
            return boxes;
        }
    } // namespace archetypes
} // namespace rerun
