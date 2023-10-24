#include "../half.hpp"
#include "bar_chart.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // [CODEGEN COPY TO HEADER START]

        BarChart(rerun::datatypes::TensorBuffer buffer) {
            this->values = rerun::components::TensorData({1}, std::move(buffer));
        }

        // --------------------------------------------------------------------
        // Implicit constructors:
        // TODO(#3794): don't use std::vector here

        /// Construct a `BarChart` from a `std::vector<uint8_t>`.
        BarChart(std::vector<uint8_t> u8)
            : BarChart(rerun::datatypes::TensorBuffer::u8(std::move(u8))) {}

        /// Construct a `BarChart` from a `std::vector<uint16_t>`.
        BarChart(std::vector<uint16_t> u16)
            : BarChart(rerun::datatypes::TensorBuffer::u16(std::move(u16))) {}

        /// Construct a `BarChart` from a `std::vector<uint32_t>`.
        BarChart(std::vector<uint32_t> u32)
            : BarChart(rerun::datatypes::TensorBuffer::u32(std::move(u32))) {}

        /// Construct a `BarChart` from a `std::vector<uint64_t>`.
        BarChart(std::vector<uint64_t> u64)
            : BarChart(rerun::datatypes::TensorBuffer::u64(std::move(u64))) {}

        /// Construct a `BarChart` from a `std::vector<int8_t>`.
        BarChart(std::vector<int8_t> i8)
            : BarChart(rerun::datatypes::TensorBuffer::i8(std::move(i8))) {}

        /// Construct a `BarChart` from a `std::vector<int16_t>`.
        BarChart(std::vector<int16_t> i16)
            : BarChart(rerun::datatypes::TensorBuffer::i16(std::move(i16))) {}

        /// Construct a `BarChart` from a `std::vector<int32_t>`.
        BarChart(std::vector<int32_t> i32)
            : BarChart(rerun::datatypes::TensorBuffer::i32(std::move(i32))) {}

        /// Construct a `BarChart` from a `std::vector<int64_t>`.
        BarChart(std::vector<int64_t> i64)
            : BarChart(rerun::datatypes::TensorBuffer::i64(std::move(i64))) {}

        /// Construct aBarChart` from a `std::vector<half>`.
        BarChart(std::vector<rerun::half> f16)
            : BarChart(rerun::datatypes::TensorBuffer::f16(std::move(f16))) {}

        /// Construct a `BarChart` from a `std::vector<float>`.
        BarChart(std::vector<float> f32)
            : BarChart(rerun::datatypes::TensorBuffer::f32(std::move(f32))) {}

        /// Construct a `BarChart` from a `std::vector<double>`.
        BarChart(std::vector<double> f64)
            : BarChart(rerun::datatypes::TensorBuffer::f64(std::move(f64))) {}

        // --------------------------------------------------------------------
        // Explicit static constructors:
        // TODO(#3794): don't use std::vector here

        /// Construct a `BarChart` from a `std::vector<uint8_t>`.
        static BarChart u8(std::vector<uint8_t> u8) {
            return BarChart(u8);
        }

        /// Construct a `BarChart` from a `std::vector<uint16_t>`.
        static BarChart u16(std::vector<uint16_t> u16) {
            return BarChart(u16);
        }

        /// Construct a `BarChart` from a `std::vector<uint32_t>`.
        static BarChart u32(std::vector<uint32_t> u32) {
            return BarChart(u32);
        }

        /// Construct a `BarChart` from a `std::vector<uint64_t>`.
        static BarChart u64(std::vector<uint64_t> u64) {
            return BarChart(u64);
        }

        /// Construct a `BarChart` from a `std::vector<int8_t>`.
        static BarChart i8(std::vector<int8_t> i8) {
            return BarChart(i8);
        }

        /// Construct a `BarChart` from a `std::vector<int16_t>`.
        static BarChart i16(std::vector<int16_t> i16) {
            return BarChart(i16);
        }

        /// Construct a `BarChart` from a `std::vector<int32_t>`.
        static BarChart i32(std::vector<int32_t> i32) {
            return BarChart(i32);
        }

        /// Construct a `BarChart` from a `std::vector<int64_t>`.
        static BarChart i64(std::vector<int64_t> i64) {
            return BarChart(i64);
        }

        /// Construct a `BarChart` from a  `std::vector<half>`.
        static BarChart f16(std::vector<rerun::half> f16) {
            return BarChart(f16);
        }

        /// Construct a `BarChart` from a `std::vector<float>`.
        static BarChart f32(std::vector<float> f32) {
            return BarChart(f32);
        }

        /// Construct a `BarChart` from a `std::vector<double>`.
        static BarChart f64(std::vector<double> f64) {
            return BarChart(f64);
        }

        // [CODEGEN COPY TO HEADER END]
#endif
    } // namespace archetypes
} // namespace rerun
