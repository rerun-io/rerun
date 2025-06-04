#if 0

#include "cylinders3d.hpp"

namespace rerun::archetypes {
    // <CODEGEN_COPY_TO_HEADER>

    /// Creates a new `Cylinder3D` with the given axis-aligned lengths and radii.
    ///
    /// For multiple cylinders, you should generally follow this with
    /// `Cylinder3D::with_centers()` and one of the rotation methods, in order to move them
    /// apart from each other.
    //
    // TODO(andreas): This should not take an std::vector.
    static Cylinder3D from_lengths_and_radii(
        const std::vector<float>& lengths, const std::vector<float>& radii
    ) {
        return Cylinder3D().with_lengths(std::move(lengths)).with_radii(std::move(radii));
    }

    // </CODEGEN_COPY_TO_HEADER>
} // namespace rerun::archetypes

#endif
