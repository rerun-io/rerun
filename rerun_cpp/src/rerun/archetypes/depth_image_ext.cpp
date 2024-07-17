#include "../error.hpp"
#include "depth_image.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

#include "../image_utils.hpp"

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, const TElement* pixels)
        : DepthImage{resolution_, get_data_type(pixels), reinterpret_cast<const uint8_t*>(pixels)} {
    }

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, std::vector<TElement> pixels)
        : DepthImage{resolution_, Collection<TElement>::take_ownership(std::move(pixels))} {}

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, Collection<TElement> pixels)
        : DepthImage{resolution_, get_data_type(pixels.data()), pixels.to_uint8()} {}

    /// New depth image from an `ChannelDataType` and a pointer.
    ///
    /// The length of the data should be `W * H * data_type.size`
    DepthImage(
        components::Resolution2D resolution_, components::ChannelDataType data_type_,
        const void* data_
    )
        : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, data_type_))},
          resolution{resolution_},
          data_type{data_type_} {}

    /// New depth image from an `ChannelDataType` and a pointer.
    ///
    /// The length of the data should be `W * H * data_type.size`
    DepthImage(
        components::Resolution2D resolution_, components::ChannelDataType data_type_,
        Collection<uint8_t> data_
    )
        : data{data_}, resolution{resolution_}, data_type{data_type_} {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
