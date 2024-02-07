#include "marker_shape.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct MarkerShapeExt {
            uint8_t shape;
#define MarkerShape MarkerShapeExt

            // <CODEGEN_COPY_TO_HEADER>

            static const rerun::components::MarkerShape CIRCLE;
            static const rerun::components::MarkerShape DIAMOND;
            static const rerun::components::MarkerShape SQUARE;
            static const rerun::components::MarkerShape CROSS;
            static const rerun::components::MarkerShape PLUS;
            static const rerun::components::MarkerShape UP;
            static const rerun::components::MarkerShape DOWN;
            static const rerun::components::MarkerShape LEFT;
            static const rerun::components::MarkerShape RIGHT;
            static const rerun::components::MarkerShape ASTERISK;

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
        const MarkerShape MarkerShape::CIRCLE = MarkerShape(1);
        const MarkerShape MarkerShape::DIAMOND = MarkerShape(2);
        const MarkerShape MarkerShape::SQUARE = MarkerShape(3);
        const MarkerShape MarkerShape::CROSS = MarkerShape(4);
        const MarkerShape MarkerShape::PLUS = MarkerShape(5);
        const MarkerShape MarkerShape::UP = MarkerShape(6);
        const MarkerShape MarkerShape::DOWN = MarkerShape(7);
        const MarkerShape MarkerShape::LEFT = MarkerShape(8);
        const MarkerShape MarkerShape::RIGHT = MarkerShape(9);
        const MarkerShape MarkerShape::ASTERISK = MarkerShape(10);

    } // namespace components
} // namespace rerun
