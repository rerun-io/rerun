// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/geo_line_string.fbs".

#pragma once

#include "../collection.hpp"
#include "../datatypes/dvec2d.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A geospatial line string expressed in EPSG:4326 latitude and longitude.
    struct GeoLineString {
        rerun::Collection<rerun::datatypes::DVec2D> lat_lon;

      public:
        GeoLineString() = default;

        GeoLineString(rerun::Collection<rerun::datatypes::DVec2D> lat_lon_)
            : lat_lon(std::move(lat_lon_)) {}

        GeoLineString& operator=(rerun::Collection<rerun::datatypes::DVec2D> lat_lon_) {
            lat_lon = std::move(lat_lon_);
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::GeoLineString> {
        static constexpr const char Name[] = "rerun.components.GeoLineString";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::GeoLineString` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::GeoLineString* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const components::GeoLineString* elements,
            size_t num_elements
        );
    };
} // namespace rerun
