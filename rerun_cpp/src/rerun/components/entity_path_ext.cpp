#if 0

#include "entity_path.hpp"

namespace rerun::components {

    // <CODEGEN_COPY_TO_HEADER>
    EntityPath(std::string_view path_) : value(std::string(path_)) {}

    EntityPath(const char* path_) : value(std::string(path_)) {}
    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::components
#endif
