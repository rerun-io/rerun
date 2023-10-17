#include "../error.hpp"
#include "image.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// New image from tensor data.
        ///
        /// Sets dimensions to width/height if they are not specified.
        /// Calls Error::handle() if the shape is not rank 2.
        explicit Image(rerun::components::TensorData _data);
        // [CODEGEN COPY TO HEADER END]
#endif

        Image::Image(rerun::components::TensorData _data) : data(std::move(_data)) {
            auto& shape = data.data.shape;
            if (shape.size() != 2 && shape.size() != 3) {
                Error(
                    ErrorCode::InvalidTensorDimension,
                    "Image shape is expected to be either rank 2 or 3."
                )
                    .handle();
                return;
            }

            if (!shape[0].name.has_value()) {
                shape[0].name = "height";
            }
            if (!shape[1].name.has_value()) {
                shape[1].name = "width";
            }
            if (!shape[2].name.has_value()) {
                shape[2].name = "depth";
            }
        }

    } // namespace archetypes
} // namespace rerun
