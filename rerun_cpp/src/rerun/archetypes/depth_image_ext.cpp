#include "../error.hpp"
#include "depth_image.hpp"

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

#include "../image_utils.hpp"

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, const TElement* pixels)
        : DepthImage{
              resolution_, get_element_type(pixels), reinterpret_cast<const uint8_t*>(pixels)} {}

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, std::vector<TElement> pixels)
        : DepthImage{resolution_, Collection<TElement>::take_ownership(std::move(pixels))} {}

    template <typename TElement>
    DepthImage(components::Resolution2D resolution_, Collection<TElement> pixels)
        : DepthImage{resolution_, get_element_type(pixels.data()), pixels.to_uint8()} {}

    /// New depth image from an `ElementType` and a pointer.
    ///
    /// The length of the data should be `W * H * element_type.size`
    DepthImage(
        components::Resolution2D resolution_, components::ElementType element_type_,
        const void* data_
    )
        : data{Collection<uint8_t>::borrow(data_, num_bytes(resolution_, element_type_))},
          resolution{resolution_},
          element_type{element_type_} {}

    /// New depth image from an `ElementType` and a pointer.
    ///
    /// The length of the data should be `W * H * element_type.size`
    DepthImage(
        components::Resolution2D resolution_, components::ElementType element_type_,
        Collection<uint8_t> data_
    )
        : data{data_}, resolution{resolution_}, element_type{element_type_} {}

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
