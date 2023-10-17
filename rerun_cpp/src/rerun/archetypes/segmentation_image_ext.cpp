#include "../error.hpp"
#include "segmentation_image.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        /// New segmentation image from tensor data.
        ///
        /// Sets dimensions to width/height if they are not specified.
        /// Calls Error::handle() if the shape is not rank 2.
        explicit SegmentationImage(rerun::components::TensorData _data);
        // [CODEGEN COPY TO HEADER END]
#endif

        SegmentationImage::SegmentationImage(rerun::components::TensorData _data)
            : data(std::move(_data)) {
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
