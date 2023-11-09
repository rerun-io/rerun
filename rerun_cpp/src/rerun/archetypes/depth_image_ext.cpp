#include "../error.hpp"
#include "depth_image.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// New depth image from height/width and tensor buffer.
        ///
        /// Sets the dimension names to "height" and "width" if they are not specified.
        /// Calls `Error::handle()` if the shape is not rank 2.
        DepthImage(std::vector<datatypes::TensorDimension> shape, datatypes::TensorBuffer buffer)
            : DepthImage(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

        /// New depth image from tensor data.
        ///
        /// Sets the dimension names to "height" and "width" if they are not specified.
        /// Calls `Error::handle()` if the shape is not rank 2.
        explicit DepthImage(components::TensorData _data);

        // </CODEGEN_COPY_TO_HEADER>
#endif

        DepthImage::DepthImage(components::TensorData _data) : data(std::move(_data)) {
            auto& shape = data.data.shape;
            if (shape.size() != 2) {
                Error(ErrorCode::InvalidTensorDimension, "Shape must be rank 2.").handle();
                return;
            }

            if (!shape[0].name.has_value()) {
                shape[0].name = "height";
            }
            if (!shape[1].name.has_value()) {
                shape[1].name = "width";
            }
        }

    } // namespace archetypes
} // namespace rerun
