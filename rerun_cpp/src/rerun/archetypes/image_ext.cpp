#include "../error.hpp"
#include "image.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// New Image from height/width/channel and tensor buffer.
        ///
        /// Sets the dimension names to "height",  "width" and "channel" if they are not specified.
        /// Calls `Error::handle()` if the shape is not rank 2 or 3.
        Image(
            std::vector<datatypes::TensorDimension> shape,
            datatypes::TensorBuffer buffer
        )
            : Image(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// New depth image from tensor data.
        ///
        /// Sets the dimension names to "height",  "width" and "channel" if they are not specified.
        /// Calls `Error::handle()` if the shape is not rank 2 or 3.
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
            if (shape.size() == 3 && shape[2].size != 1 && shape[2].size != 3 &&
                shape[2].size != 4) {
                Error(
                    ErrorCode::InvalidTensorDimension,
                    "Only images with 1, 3 and 4 channels are supported."
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
            if (shape.size() > 2 && !shape[2].name.has_value()) {
                shape[2].name = "depth";
            }
        }

    } // namespace archetypes
} // namespace rerun
