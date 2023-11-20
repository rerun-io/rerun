#include "../half.hpp"
#include "bar_chart.hpp"

namespace rerun::archetypes {

#if 0
        // <CODEGEN_COPY_TO_HEADER>

        BarChart(rerun::datatypes::TensorBuffer buffer) {
            auto num_elems = buffer.num_elems();
            this->values = rerun::components::TensorData({num_elems}, std::move(buffer));
        }

        // --------------------------------------------------------------------
        // Implicit constructors:

        /// Construct a `BarChart` from a `Collection<uint8_t>`.
        BarChart(Collection<uint8_t> u8)
            : BarChart(rerun::datatypes::TensorBuffer::u8(std::move(u8))) {}

        /// Construct a `BarChart` from a `Collection<uint16_t>`.
        BarChart(Collection<uint16_t> u16)
            : BarChart(rerun::datatypes::TensorBuffer::u16(std::move(u16))) {}

        /// Construct a `BarChart` from a `Collection<uint32_t>`.
        BarChart(Collection<uint32_t> u32)
            : BarChart(rerun::datatypes::TensorBuffer::u32(std::move(u32))) {}

        /// Construct a `BarChart` from a `Collection<uint64_t>`.
        BarChart(Collection<uint64_t> u64)
            : BarChart(rerun::datatypes::TensorBuffer::u64(std::move(u64))) {}

        /// Construct a `BarChart` from a `Collection<int8_t>`.
        BarChart(Collection<int8_t> i8)
            : BarChart(rerun::datatypes::TensorBuffer::i8(std::move(i8))) {}

        /// Construct a `BarChart` from a `Collection<int16_t>`.
        BarChart(Collection<int16_t> i16)
            : BarChart(rerun::datatypes::TensorBuffer::i16(std::move(i16))) {}

        /// Construct a `BarChart` from a `Collection<int32_t>`.
        BarChart(Collection<int32_t> i32)
            : BarChart(rerun::datatypes::TensorBuffer::i32(std::move(i32))) {}

        /// Construct a `BarChart` from a `Collection<int64_t>`.
        BarChart(Collection<int64_t> i64)
            : BarChart(rerun::datatypes::TensorBuffer::i64(std::move(i64))) {}

        /// Construct aBarChart` from a `Collection<half>`.
        BarChart(Collection<rerun::half> f16)
            : BarChart(rerun::datatypes::TensorBuffer::f16(std::move(f16))) {}

        /// Construct a `BarChart` from a `Collection<float>`.
        BarChart(Collection<float> f32)
            : BarChart(rerun::datatypes::TensorBuffer::f32(std::move(f32))) {}

        /// Construct a `BarChart` from a `Collection<double>`.
        BarChart(Collection<double> f64)
            : BarChart(rerun::datatypes::TensorBuffer::f64(std::move(f64))) {}

        // --------------------------------------------------------------------
        // Explicit static constructors:

        /// Construct a `BarChart` from a `Collection<uint8_t>`.
        static BarChart u8(Collection<uint8_t> u8) {
            return BarChart(std::move(u8));
        }

        /// Construct a `BarChart` from a `Collection<uint16_t>`.
        static BarChart u16(Collection<uint16_t> u16) {
            return BarChart(std::move(u16));
        }

        /// Construct a `BarChart` from a `Collection<uint32_t>`.
        static BarChart u32(Collection<uint32_t> u32) {
            return BarChart(std::move(u32));
        }

        /// Construct a `BarChart` from a `Collection<uint64_t>`.
        static BarChart u64(Collection<uint64_t> u64) {
            return BarChart(std::move(u64));
        }

        /// Construct a `BarChart` from a `Collection<int8_t>`.
        static BarChart i8(Collection<int8_t> i8) {
            return BarChart(std::move(i8));
        }

        /// Construct a `BarChart` from a `Collection<int16_t>`.
        static BarChart i16(Collection<int16_t> i16) {
            return BarChart(std::move(i16));
        }

        /// Construct a `BarChart` from a `Collection<int32_t>`.
        static BarChart i32(Collection<int32_t> i32) {
            return BarChart(std::move(i32));
        }

        /// Construct a `BarChart` from a `Collection<int64_t>`.
        static BarChart i64(Collection<int64_t> i64) {
            return BarChart(std::move(i64));
        }

        /// Construct a `BarChart` from a  `Collection<half>`.
        static BarChart f16(Collection<rerun::half> f16) {
            return BarChart(std::move(f16));
        }

        /// Construct a `BarChart` from a `Collection<float>`.
        static BarChart f32(Collection<float> f32) {
            return BarChart(std::move(f32));
        }

        /// Construct a `BarChart` from a `Collection<double>`.
        static BarChart f64(Collection<double> f64) {
            return BarChart(std::move(f64));
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
} // namespace rerun::archetypes
