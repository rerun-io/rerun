#include "arrows3d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Creates new 3D arrows pointing in the given directions.
        static Arrows3D from_vectors(std::vector<components::Vector3D> _vectors) {
            Arrows3D arrows;
            arrows.vectors = std::move(_vectors);
            return arrows;
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
