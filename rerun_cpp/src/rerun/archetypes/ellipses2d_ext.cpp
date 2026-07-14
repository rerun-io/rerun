#include "ellipses2d.hpp"

#include "../collection_adapter_builtins.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates new `Ellipses2D` with `half_sizes` centered around the local origin.
        static Ellipses2D from_half_sizes(Collection<components::HalfSize2D> half_sizes) {
            return Ellipses2D().with_half_sizes(std::move(half_sizes));
        }

        /// Creates new `Ellipses2D` with `centers` and `half_sizes`.
        static Ellipses2D from_centers_and_half_sizes(
            Collection<components::Position2D> centers,
            Collection<components::HalfSize2D> half_sizes
        ) {
            return Ellipses2D()
                .with_half_sizes(std::move(half_sizes))
                .with_centers(std::move(centers));
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace archetypes
} // namespace rerun
