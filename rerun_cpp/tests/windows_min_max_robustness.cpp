
// `<windows.h>` by default defined min and max as macros, which can cause issues with std::min and std::max.
// This can be easily turned off by the user with `#define NOMINMAX` before including `<windows.h>`.
// However, users might not know about this or forget about it and we want to be robust against it.
//
// This test checks that rerun.h is robust against this issue by explicitly setting `min`/`max` macros and including rerun.hpp.
#define min(a, b) (this does not compile)
#define max(a, b) (this does not compile)

#include <rerun.hpp>
