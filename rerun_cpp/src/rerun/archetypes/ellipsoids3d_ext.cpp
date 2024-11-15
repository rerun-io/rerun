#include "ellipsoids3d.hpp"

#include "../collection_adapter_builtins.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new `Ellipsoids3D` that are spheres, with `half_sizes` created from radii.
        //
        // TODO(andreas): This should not take an std::vector.
        static Ellipsoids3D from_radii(const std::vector<float>& sizes);

        /// Creates new `Ellipsoids3D` that are spheres, with `half_sizes` and `centers` created
        /// from centers and radii.
        //
        // TODO(andreas): This should not take an std::vector.
        static Ellipsoids3D from_centers_and_radii(
            const std::vector<datatypes::Vec3D>& centers, const std::vector<float>& radii
        );

        /// Creates new `Ellipsoids3D` with `half_sizes` centered around the local origin.
        static Ellipsoids3D from_half_sizes(Collection<components::HalfSize3D> half_sizes) {
            Ellipsoids3D ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            return ellipsoids;
        }

        /// Creates new `Ellipsoids3D` with `centers` and `half_sizes`.
        static Ellipsoids3D from_centers_and_half_sizes(
            Collection<components::PoseTranslation3D> centers,
            Collection<components::HalfSize3D> half_sizes
        ) {
            Ellipsoids3D ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            ellipsoids.centers = std::move(centers);
            return ellipsoids;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
        Ellipsoids3D Ellipsoids3D::from_radii(const std::vector<float>& radii) {
            std::vector<components::HalfSize3D> half_sizes;
            half_sizes.reserve(radii.size());
            for (const auto& radius : radii) {
                half_sizes.emplace_back(radius, radius, radius);
            }

            // Move the vector into a component batch.
            return Ellipsoids3D::from_half_sizes(std::move(half_sizes));
        }

        Ellipsoids3D Ellipsoids3D::from_centers_and_radii(
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
            Ellipsoids3D ellipsoids;
            ellipsoids.half_sizes = std::move(half_sizes);
            ellipsoids.centers = std::move(centers);
            return ellipsoids;
        }
    } // namespace archetypes
} // namespace rerun
