#include "arrows3d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new 3D arrows pointing in the given directions, with a base at the origin (0, 0,
        /// 0).
        static Arrows3D from_vectors(Collection<components::Vector3D> vectors_) {
            return Arrows3D().with_vectors(vectors_);
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace archetypes
} // namespace rerun
