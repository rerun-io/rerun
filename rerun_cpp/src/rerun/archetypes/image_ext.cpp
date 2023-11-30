#include "../error.hpp"
#include "image.hpp"

#include "../collection_adapter_builtins.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

    /// New Image from height/width/channel and tensor buffer.
    ///
    /// \param shape
    /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2 or 3.
    /// Sets the dimension names to "height", "width" and "channel" if they are not specified.
    /// \param buffer
    /// The tensor buffer containing the image data.
    explicit Image(Collection<datatypes::TensorDimension> shape, datatypes::TensorBuffer buffer)
        : Image(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

    /// New depth image from tensor data.
    ///
    /// \param data_
    /// The tensor buffer containing the image data.
    /// Sets the dimension names to "height",  "width" and "channel" if they are not specified.
    /// Calls `Error::handle()` if the shape is not rank 2 or 3.
    explicit Image(rerun::components::TensorData data_);

    /// New image from dimensions and pointer to image data.
    ///
    /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
    /// \param shape
    /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2 or 3.
    /// Sets the dimension names to "height", "width" and "channel" if they are not specified.
    /// Determines the number of elements expected to be in `data`.
    /// \param data_
    /// Target of the pointer must outlive the archetype.
    template <typename TElement>
    explicit Image(Collection<datatypes::TensorDimension> shape, const TElement* data_)
        : Image(datatypes::TensorData(std::move(shape), data_)) {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

    Image::Image(rerun::components::TensorData data_) : data(std::move(data_)) {
        auto& shape = data.data.shape;
        if (shape.size() != 2 && shape.size() != 3) {
            Error(
                ErrorCode::InvalidTensorDimension,
                "Image shape is expected to be either rank 2 or 3."
            )
                .handle();
            return;
        }
        if (shape.size() == 3 && shape[2].size != 1 && shape[2].size != 3 && shape[2].size != 4) {
            Error(
                ErrorCode::InvalidTensorDimension,
                "Only images with 1, 3 and 4 channels are supported."
            )
                .handle();
            return;
        }

        // We want to change the dimension names if they are not specified.
        // But rerun collections are strictly immutable, so create a new one if necessary.
        bool overwrite_height = !shape[0].name.has_value();
        bool overwrite_width = !shape[1].name.has_value();
        bool overwrite_depth = shape.size() > 2 && !shape[2].name.has_value();

        if (overwrite_height || overwrite_width || overwrite_depth) {
            auto new_shape = shape.to_vector();

            if (overwrite_height) {
                new_shape[0].name = "height";
            }
            if (overwrite_width) {
                new_shape[1].name = "width";
            }
            if (overwrite_depth) {
                new_shape[2].name = "depth";
            }

            shape = std::move(new_shape);
        }
    }
} // namespace rerun::archetypes
