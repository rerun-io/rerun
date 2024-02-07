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

            static const rerun::components::MarkerShape Circle;
            static const rerun::components::MarkerShape Diamond;
            static const rerun::components::MarkerShape Square;
            static const rerun::components::MarkerShape Cross;
            static const rerun::components::MarkerShape Plus;
            static const rerun::components::MarkerShape Up;
            static const rerun::components::MarkerShape Down;
            static const rerun::components::MarkerShape Left;
            static const rerun::components::MarkerShape Right;
            static const rerun::components::MarkerShape Asterisk;

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
        // TODO(#3384): This should be generated
        const MarkerShape MarkerShape::Circle = MarkerShape(1);
        const MarkerShape MarkerShape::Diamond = MarkerShape(2);
        const MarkerShape MarkerShape::Square = MarkerShape(3);
        const MarkerShape MarkerShape::Cross = MarkerShape(4);
        const MarkerShape MarkerShape::Plus = MarkerShape(5);
        const MarkerShape MarkerShape::Up = MarkerShape(6);
        const MarkerShape MarkerShape::Down = MarkerShape(7);
        const MarkerShape MarkerShape::Left = MarkerShape(8);
        const MarkerShape MarkerShape::Right = MarkerShape(9);
        const MarkerShape MarkerShape::Asterisk = MarkerShape(10);

    } // namespace components
} // namespace rerun
