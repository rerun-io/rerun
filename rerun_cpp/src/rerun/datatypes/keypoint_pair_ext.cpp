#include <utility>
#include "keypoint_pair.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct KeypointPairExt {
            rerun::components::KeypointId keypoint0;
            rerun::components::KeypointId keypoint1;

#define KeypointPair KeypointPairExt

            // <CODEGEN_COPY_TO_HEADER>

            KeypointPair(uint16_t _keypoint0, uint16_t _keypoint1)
                : keypoint0(_keypoint0), keypoint1(_keypoint1) {}

            KeypointPair(std::pair<uint16_t, uint16_t> pair)
                : keypoint0(pair.first), keypoint1(pair.second) {}

            // </CODEGEN_COPY_TO_HEADER>
        };

#endif
    } // namespace datatypes
} // namespace rerun
