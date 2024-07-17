#include "../error.hpp"
#include "depth_image.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

#include "../image_utils.hpp"

    template <typename TElement>
    DepthImage(const TElement* pixels, components::Resolution2D resolution_)
        : DepthImage{reinterpret_cast<const uint8_t*>(pixels), resolution_, get_data_type(pixels)} {
    }

    template <typename TElement>
    DepthImage(std::vector<TElement> pixels, components::Resolution2D resolution_)
        : DepthImage{Collection<TElement>::take_ownership(std::move(pixels)), resolution_} {}

    template <typename TElement>
    DepthImage(Collection<TElement> pixels, components::Resolution2D resolution_)
        : DepthImage{pixels.to_uint8(), resolution_, get_data_type(pixels.data())} {}

    /// New depth image from an `ChannelDataType` and a pointer.
    ///
    /// The length of the data should be `W * H * data_type.size`
    DepthImage(
        const void* data_, components::Resolution2D resolution_,
        components::ChannelDataType data_type_
    )
        : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, data_type_))},
          resolution{resolution_},
          data_type{data_type_} {}

    /// New depth image from an `ChannelDataType` and a pointer.
    ///
    /// The length of the data should be `W * H * data_type.size`
    DepthImage(
        Collection<uint8_t> data_, components::Resolution2D resolution_,
        components::ChannelDataType data_type_
    )
        : data{data_}, resolution{resolution_}, data_type{data_type_} {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
