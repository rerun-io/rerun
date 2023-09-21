#include "mesh_properties.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct MeshPropertiesExt {
#define MeshProperties MeshPropertiesExt

            // [CODEGEN COPY TO HEADER START]

            static MeshProperties from_triangle_indices(std::vector<uint32_t> vertex_indices) {
                return MeshProperties(vertex_indices);
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
