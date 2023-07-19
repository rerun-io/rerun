// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/mat3x3.fbs"

#pragma once

#include <cstdint>
#include <memory>

namespace arrow {
    class DataType;
}

namespace rr {
    namespace datatypes {
        /// A 3x3 column-major Matrix.
        struct Mat3x3 {
            float coeffs[9];

          public:
            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();
        };
    } // namespace datatypes
} // namespace rr
