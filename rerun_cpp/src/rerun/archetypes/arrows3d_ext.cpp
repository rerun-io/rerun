#include "arrows3d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Creates new 3D arrows pointing in the given directions, with a base at the origin (0, 0,
        /// 0).
        static Arrows3D from_vectors(std::vector<components::Vector3D> _vectors) {
            Arrows3D arrows;
            arrows.vectors = std::move(_vectors);
            return arrows;
        }

        /// Creates new 3D arrows pointing in the given directions, with a base at the origin (0, 0,
        /// 0).
        static Arrows3D from_vectors(components::Vector3D _vector) {
            Arrows3D arrows;
            arrows.vectors = std::vector(1, std::move(_vector));
            return arrows;
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
