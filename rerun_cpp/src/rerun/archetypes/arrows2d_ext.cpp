#include "arrows2d.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new 2D arrows pointing in the given directions, with a base at the origin (0, 0).
        static Arrows2D from_vectors(Collection<components::Vector2D> vectors_) {
            return Arrows2D().with_vectors(vectors_);
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace archetypes
} // namespace rerun
