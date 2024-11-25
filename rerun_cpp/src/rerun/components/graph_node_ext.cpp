//#define EDIT_EXTENSION

#ifdef EDIT_EXTENSION
#include "graph_node.hpp"

namespace rerun {
    namespace archetypes {

        // <CODEGEN_COPY_TO_HEADER>

        /// Create a new graph edge from a c string.
        GraphNode(const char* value_) : id(value_) {}

        // </CODEGEN_COPY_TO_HEADER>

    } // namespace archetypes
} // namespace rerun

#endif
