// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:53.
// Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

#pragma once

#include "../arrow.hpp"
#include "../components/annotation_context.hpp"
#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// The `AnnotationContext` provides additional information on how to display entities.
        ///
        /// Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
        /// the labels and colors will be looked up in the appropriate
        ///`AnnotationContext`. We use the *first* annotation context we find in the
        /// path-hierarchy when searching up through the ancestors of a given entity
        /// path.
        ///
        /// ## Example
        ///
        ///```
        ///// Log an annotation context to assign a label and color to each class
        ///
        /// #include <rerun.hpp>
        ///
        /// namespace rr = rerun;
        ///
        /// int main() {
        ///    auto rec = rr::RecordingStream("rerun_example_annotation_context_rects");
        ///    rec.connect("127.0.0.1:9876").throw_on_failure();
        ///
        ///    // Log an annotation context to assign a label and color to each class
        ///    rec.log(
        ///        "/",
        ///        rr::AnnotationContext({
        ///            rr::datatypes::AnnotationInfo(1, "red", rr::datatypes::Color(255, 0, 0)),
        ///            rr::datatypes::AnnotationInfo(2, "green", rr::datatypes::Color(0, 255, 0)),
        ///        })
        ///    );
        ///
        ///    // Log a batch of 2 arrows with different `class_ids`
        ///    rec.log(
        ///        "arrows",
        ///        rr::Arrows3D({{1.0f, 0.0f, 0.0f}, {0.0f, 1.0f, 0.0f}}).with_class_ids({1, 2})
        ///    );
        /// }
        ///```
        struct AnnotationContext {
            rerun::components::AnnotationContext context;

          public:
            AnnotationContext() = default;

            AnnotationContext(rerun::components::AnnotationContext _context)
                : context(std::move(_context)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// Creates a list of Rerun DataCell from this archetype.
            Result<std::vector<rerun::DataCell>> to_data_cells() const;
        };
    } // namespace archetypes
} // namespace rerun
