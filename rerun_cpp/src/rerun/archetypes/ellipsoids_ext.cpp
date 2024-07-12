#include "ellipsoids.hpp"

#include "../collection_adapter_builtins.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new `Ellipsoids` with `half_sizes` centered around the local origin.
        static Ellipsoids from_half_sizes(Collection<components::HalfSize3D> half_sizes) {
            Ellipsoids ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            return ellipsoids;
        }

        /// Creates new `Ellipsoids` with `half_sizes` created from radii.
        //
        // TODO(andreas): This should not take an std::vector.
        static Ellipsoids from_radii(const std::vector<float>& sizes);

        /// Creates new `Ellipsoids` with `centers` and `half_sizes`.
        static Ellipsoids from_centers_and_half_sizes(
            Collection<components::Position3D> centers,
            Collection<components::HalfSize3D> half_sizes
        ) {
            Ellipsoids ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            ellipsoids.centers = std::move(centers);
            return ellipsoids;
        }

        /// Creates new `Ellipsoids` with `half_sizes` and `centers` created from centers and radii.
        //
        // TODO(andreas): This should not take an std::vector.
        static Ellipsoids from_centers_and_radii(
            const std::vector<datatypes::Vec3D>& centers, const std::vector<float>& radii
        );

        // </CODEGEN_COPY_TO_HEADER>
#endif
        Ellipsoids Ellipsoids::from_radii(const std::vector<float>& radii) {
            std::vector<components::HalfSize3D> half_sizes;
            half_sizes.reserve(radii.size());
            for (const auto& radius : radii) {
                half_sizes.emplace_back(radius, radius, radius);
            }

            // Move the vector into a component batch.
            return Ellipsoids::from_half_sizes(std::move(half_sizes));
        }

        Ellipsoids Ellipsoids::from_centers_and_radii(
            const std::vector<datatypes::Vec3D>& centers, const std::vector<float>& radii
        ) {
            auto num_components = std::min(centers.size(), radii.size());

            std::vector<components::HalfSize3D> half_sizes;
            half_sizes.reserve(num_components);

            for (size_t i = 0; i < num_components; ++i) {
                float radius = radii[i];
                half_sizes.emplace_back(radius, radius, radius);
            }

            // We only transformed the radii; the centers are good as-is.
            Ellipsoids ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            ellipsoids.centers = std::move(centers);
            return ellipsoids;
        }
    } // namespace archetypes
} // namespace rerun
