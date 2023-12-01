#include "../error.hpp"
#include "depth_image.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

    /// New depth image from height/width and tensor buffer.
    ///
    /// \param shape
    /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2.
    /// Sets the dimension names to "height" and "width" if they are not specified.
    /// \param buffer
    /// The tensor buffer containing the depth image data.
    DepthImage(Collection<datatypes::TensorDimension> shape, datatypes::TensorBuffer buffer)
        : DepthImage(datatypes::TensorData(std::move(shape), std::move(buffer))) {}

    /// New depth image from tensor data.
    ///
    /// \param data_
    /// The tensor buffer containing the depth image data.
    /// Sets the dimension names to "height" and "width" if they are not specified.
    /// Calls `Error::handle()` if the shape is not rank 2.
    explicit DepthImage(components::TensorData data_);

    /// New depth image from dimensions and pointer to depth image data.
    ///
    /// Type must be one of the types supported by `rerun::datatypes::TensorData`.
    /// \param shape
    /// Shape of the image. Calls `Error::handle()` if the shape is not rank 2.
    /// Sets the dimension names to "height", "width" and "channel" if they are not specified.
    /// Determines the number of elements expected to be in `data`.
    /// \param data_
    /// Target of the pointer must outlive the archetype.
    template <typename TElement>
    explicit DepthImage(Collection<datatypes::TensorDimension> shape, const TElement* data_)
        : DepthImage(datatypes::TensorData(std::move(shape), data_)) {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

    DepthImage::DepthImage(components::TensorData data_) : data(std::move(data_)) {
        auto& shape = data.data.shape;
        if (shape.size() != 2) {
            Error(ErrorCode::InvalidTensorDimension, "Shape must be rank 2.").handle();
            return;
        }

        // We want to change the dimension names if they are not specified.
        // But rerun collections are strictly immutable, so create a new one if necessary.
        bool overwrite_height = !shape[0].name.has_value();
        bool overwrite_width = !shape[1].name.has_value();

        if (overwrite_height || overwrite_width) {
            auto new_shape = shape.to_vector();

            if (overwrite_height) {
                new_shape[0].name = "height";
            }
            if (overwrite_width) {
                new_shape[1].name = "width";
            }

            shape = std::move(new_shape);
        }
    }

} // namespace rerun::archetypes
