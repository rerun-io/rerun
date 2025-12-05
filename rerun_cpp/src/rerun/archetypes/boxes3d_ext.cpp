#include "boxes3d.hpp"

#include "../collection_adapter_builtins.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new `Boxes3D` with `half_sizes` centered around the local origin.
        static Boxes3D from_half_sizes(Collection<components::HalfSize3D> half_sizes) {
            return Boxes3D().with_half_sizes(std::move(half_sizes));
        }

        /// Creates new `Boxes3D` with `centers` and `half_sizes`.
        static Boxes3D from_centers_and_half_sizes(
            Collection<components::Translation3D> centers,
            Collection<components::HalfSize3D> half_sizes
        ) {
            return Boxes3D()
                .with_half_sizes(std::move(half_sizes))
                .with_centers(std::move(centers));
        }

        /// Creates new `Boxes3D` with `half_sizes` created from (full) sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates half-sizes from the
        /// input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_sizes(const std::vector<datatypes::Vec3D>& sizes);

        /// Creates new `Boxes3D` with `centers` and `half_sizes` created from centers and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_centers_and_sizes(
            Collection<components::Translation3D> centers,
            const std::vector<datatypes::Vec3D>& sizes
        ) {
            return from_sizes(std::move(sizes)).with_centers(std::move(centers));
        }

        /// Creates new `Boxes3D` with `half_sizes` and `centers` created from minimums and (full)
        /// sizes.
        ///
        /// TODO(#3285): Does *not* preserve data as-is and instead creates centers and half-sizes
        /// from the input data.
        /// TODO(andreas): This should not take an std::vector.
        static Boxes3D from_mins_and_sizes(
            const std::vector<datatypes::Vec3D>& mins, const std::vector<datatypes::Vec3D>& sizes
        );

        // </CODEGEN_COPY_TO_HEADER>
#endif
        Boxes3D Boxes3D::from_sizes(const std::vector<datatypes::Vec3D>& sizes) {
            std::vector<components::HalfSize3D> half_sizes;
            half_sizes.reserve(sizes.size());
            for (const auto& size : sizes) {
                half_sizes.emplace_back(size.x() / 2.0f, size.y() / 2.0f, size.z() / 2.0f);
            }

            // Move the vector into a component batch.
            return Boxes3D::from_half_sizes(std::move(half_sizes));
        }

        Boxes3D Boxes3D::from_mins_and_sizes(
            const std::vector<datatypes::Vec3D>& mins, const std::vector<datatypes::Vec3D>& sizes
        ) {
            auto num_components = std::min(mins.size(), sizes.size());

            std::vector<components::HalfSize3D> half_sizes;
            std::vector<components::Translation3D> centers;
            half_sizes.reserve(num_components);
            centers.reserve(num_components);

            for (size_t i = 0; i < num_components; ++i) {
                float half_size_x = sizes[i].x() * 0.5f;
                float half_size_y = sizes[i].y() * 0.5f;
                float half_size_z = sizes[i].z() * 0.5f;

                half_sizes.emplace_back(half_size_x, half_size_y, half_size_z);
                centers.emplace_back(
                    mins[i].x() + half_size_x,
                    mins[i].y() + half_size_y,
                    mins[i].z() + half_size_z
                );
            }

            return Boxes3D()
                .with_half_sizes(std::move(half_sizes))
                .with_centers(std::move(centers));
        }
    } // namespace archetypes
} // namespace rerun
