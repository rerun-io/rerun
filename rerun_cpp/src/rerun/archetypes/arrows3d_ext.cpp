#include "arrows3d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// Creates new 3D arrows pointing in the given directions, with a base at the origin (0, 0,
        /// 0).
        static Arrows3D from_vectors(ComponentBatch<components::Vector3D> vectors_) {
            Arrows3D arrows;
            arrows.vectors = std::move(vectors_);
            return arrows;
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
