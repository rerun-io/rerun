#if 0

#include "cylinders3d.hpp"

namespace rerun::archetypes {
    // <CODEGEN_COPY_TO_HEADER>

    /// Creates a new `Cylinders3D` with the given axis-aligned lengths and radii.
    ///
    /// For multiple cylinders, you should generally follow this with
    /// `Cylinders3D::with_centers()` and one of the rotation methods, in order to move them
    /// apart from each other.
    static Cylinders3D from_lengths_and_radii(
        const Collection<rerun::components::Length>& lengths, const Collection<rerun::components::Radius>& radii) {
        return Cylinders3D().with_lengths(lengths).with_radii(radii);

    // </CODEGEN_COPY_TO_HEADER>
} // namespace rerun::archetypes

#endif
