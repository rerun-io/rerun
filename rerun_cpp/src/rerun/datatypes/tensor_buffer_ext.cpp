#include "../half.hpp"
#include "tensor_buffer.hpp"

#include <cassert>

// <CODEGEN_COPY_TO_HEADER>

#include "../type_traits.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::datatypes {

#if 0
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct a `TensorBuffer` from a `Collection<uint8_t>`.
    TensorBuffer(Collection<uint8_t> u8) : TensorBuffer(TensorBuffer::u8(std::move(u8))) {}

    /// Construct a `TensorBuffer` from a `Collection<uint16_t>`.
    TensorBuffer(Collection<uint16_t> u16)
        : TensorBuffer(TensorBuffer::u16(std::move(u16))) {}

    /// Construct a `TensorBuffer` from a `Collection<uint32_t>`.
    TensorBuffer(Collection<uint32_t> u32)
        : TensorBuffer(TensorBuffer::u32(std::move(u32))) {}

    /// Construct a `TensorBuffer` from a `Collection<uint64_t>`.
    TensorBuffer(Collection<uint64_t> u64)
        : TensorBuffer(TensorBuffer::u64(std::move(u64))) {}

    /// Construct a `TensorBuffer` from a `Collection<int8_t>`.
    TensorBuffer(Collection<int8_t> i8) : TensorBuffer(TensorBuffer::i8(std::move(i8))) {}

    /// Construct a `TensorBuffer` from a `Collection<int16_t>`.
    TensorBuffer(Collection<int16_t> i16)
        : TensorBuffer(TensorBuffer::i16(std::move(i16))) {}

    /// Construct a `TensorBuffer` from a `Collection<int32_t>`.
    TensorBuffer(Collection<int32_t> i32)
        : TensorBuffer(TensorBuffer::i32(std::move(i32))) {}

    /// Construct a `TensorBuffer` from a `Collection<int64_t>`.
    TensorBuffer(Collection<int64_t> i64)
        : TensorBuffer(TensorBuffer::i64(std::move(i64))) {}

    /// Construct a `TensorBuffer` from a `Collection<half>`.
    TensorBuffer(Collection<rerun::half> f16)
        : TensorBuffer(TensorBuffer::f16(std::move(f16))) {}

    /// Construct a `TensorBuffer` from a `Collection<float>`.
    TensorBuffer(Collection<float> f32) : TensorBuffer(TensorBuffer::f32(std::move(f32))) {}

    /// Construct a `TensorBuffer` from a `Collection<double>`.
    TensorBuffer(Collection<double> f64)
        : TensorBuffer(TensorBuffer::f64(std::move(f64))) {}

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
    /// You may NOT call this for JPEG buffers.
    size_t num_elems() const;

    // </CODEGEN_COPY_TO_HEADER>
#endif

    /// Number of elements in the buffer.
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
            case detail::TensorBufferTag::NV12: {
                assert(false && "Can't ask for the number of elements in an NV12 encoded image");
                break;
            }
            case detail::TensorBufferTag::JPEG: {
                assert(false && "Can't ask for the number of elements in a JPEG");
                break;
            }
        }
        assert(false && "Unknown TensorBuffer tag");
        return 0;
    }

} // namespace rerun::datatypes
