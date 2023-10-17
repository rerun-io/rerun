#include "../half.hpp"
#include "tensor_buffer.hpp"

#include <cassert>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct TensorBufferExt {
#define TensorBuffer TensorBufferExt

            // [CODEGEN COPY TO HEADER START]

            // TODO(#3794): don't use std::vector here

            /// Construct a `TensorBuffer` from a `std::vector<uint8_t>`.
            TensorBuffer(std::vector<uint8_t> u8) : TensorBuffer(TensorBuffer::u8(u8)) {}

            /// Construct a `TensorBuffer` from a `std::vector<uint16_t>`.
            TensorBuffer(std::vector<uint16_t> u16) : TensorBuffer(TensorBuffer::u16(u16)) {}

            /// Construct a `TensorBuffer` from a `std::vector<uint32_t>`.
            TensorBuffer(std::vector<uint32_t> u32) : TensorBuffer(TensorBuffer::u32(u32)) {}

            /// Construct a `TensorBuffer` from a `std::vector<uint64_t>`.
            TensorBuffer(std::vector<uint64_t> u64) : TensorBuffer(TensorBuffer::u64(u64)) {}

            /// Construct a `TensorBuffer` from a `std::vector<int8_t>`.
            TensorBuffer(std::vector<int8_t> i8) : TensorBuffer(TensorBuffer::i8(i8)) {}

            /// Construct a `TensorBuffer` from a `std::vector<int16_t>`.
            TensorBuffer(std::vector<int16_t> i16) : TensorBuffer(TensorBuffer::i16(i16)) {}

            /// Construct a `TensorBuffer` from a `std::vector<int32_t>`.
            TensorBuffer(std::vector<int32_t> i32) : TensorBuffer(TensorBuffer::i32(i32)) {}

            /// Construct a `TensorBuffer` from a `std::vector<int64_t>`.
            TensorBuffer(std::vector<int64_t> i64) : TensorBuffer(TensorBuffer::i64(i64)) {}

            /// Construct a `TensorBuffer` from a `std::vector<half>`.
            TensorBuffer(std::vector<rerun::half> f16) : TensorBuffer(TensorBuffer::f16(f16)) {}

            /// Construct a `TensorBuffer` from a `std::vector<float>`.
            TensorBuffer(std::vector<float> f32) : TensorBuffer(TensorBuffer::f32(f32)) {}

            /// Construct a `TensorBuffer` from a `std::vector<double>`.
            TensorBuffer(std::vector<double> f64) : TensorBuffer(TensorBuffer::f64(f64)) {}

            /// Number of elements in the buffer.
            ///
            /// You may NOT call this for JPEG buffers.
            size_t num_elems() const;

            // [CODEGEN COPY TO HEADER END]
        };

#undef TensorBuffer
#else
#define TensorBufferExt TensorBuffer
#endif

        /// Number of elements in the buffer.
        size_t TensorBufferExt::num_elems() const {
            switch (this->_tag) {
                case detail::TensorBufferTag::NONE: {
                    break; // Nothing to destroy
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
                    assert(
                        false && "Can't ask for the number of elements in an NV12 encoded image"
                    );
                }
                case detail::TensorBufferTag::JPEG: {
                    assert(false && "Can't ask for the number of elements in a JPEG");
                }
            }
            assert(false && "Unknown TensorBuffer tag");
            return 0;
        }

    } // namespace datatypes
} // namespace rerun
