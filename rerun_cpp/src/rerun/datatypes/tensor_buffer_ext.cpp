#include "../half.hpp"
#include "tensor_buffer.hpp"

#include <cassert>

// <CODEGEN_COPY_TO_HEADER>

#include "../type_traits.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::datatypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct a `TensorBuffer` from any container type that has a `value_type` member and for which a
    /// `rerun::ContainerAdapter` exists.
    ///
    /// This constructor assumes the type of tensor buffer you want to use is defined by `TContainer::value_type`
    /// and then forwards the argument as-is to the appropriate `rerun::Container` constructor.
    /// \see rerun::ContainerAdapter, rerun::Container
    template <typename TContainer, typename value_type = traits::value_type_of_t<TContainer>>
    TensorBuffer(TContainer&& container)
        : TensorBuffer(Collection<value_type>(std::forward<TContainer>(container))) {}

    /// Number of elements in the buffer.
    ///
    /// You may NOT call this for NV12 or YUY2.
    size_t num_elems() const;

    // </CODEGEN_COPY_TO_HEADER>
#endif

    size_t TensorBuffer::num_elems() const {
        switch (this->_tag) {
            case detail::TensorBufferTag::None: {
                return 0;
            }
            case detail::TensorBufferTag::U8: {
                return _data.u8.size();
            }
            case detail::TensorBufferTag::U16: {
                return _data.u16.size();
            }
            case detail::TensorBufferTag::U32: {
                return _data.u32.size();
            }
            case detail::TensorBufferTag::U64: {
                return _data.u64.size();
            }
            case detail::TensorBufferTag::I8: {
                return _data.i8.size();
            }
            case detail::TensorBufferTag::I16: {
                return _data.i16.size();
            }
            case detail::TensorBufferTag::I32: {
                return _data.i32.size();
            }
            case detail::TensorBufferTag::I64: {
                return _data.i64.size();
            }
            case detail::TensorBufferTag::F16: {
                return _data.f16.size();
            }
            case detail::TensorBufferTag::F32: {
                return _data.f32.size();
            }
            case detail::TensorBufferTag::F64: {
                return _data.f64.size();
            }
        }
        assert(false && "Unknown TensorBuffer tag");
        return 0;
    }

} // namespace rerun::datatypes
