#include "volume3d.hpp"

namespace rerun::archetypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

RR_DISABLE_MAYBE_UNINITIALIZED_PUSH

    /// New Volume3D from dimensions and tensor buffer.
    Volume3D(Collection<uint64_t> shape, encodings::TensorBuffer buffer)
        : Volume3D(encodings::TensorData(std::move(shape), std::move(buffer))) {}

RR_DISABLE_MAYBE_UNINITIALIZED_POP

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
