---
title: C++ Quick Start
order: 2
---

## Setup
Before adding Rerun to your application, start by [installing the viewer](installing-viewer.md).

The Rerun C++ SDK depends on an install of the `arrow-cpp` library on your system using.
If you are using [Pixi](https://prefix.dev/docs/pixi/overview), you can simply type `pixi global install arrow-cpp`.
Find more information about other package managers at the official Arrow Apache [install guide](https://arrow.apache.org/install/).

## Learning by example
If you prefer to learn by example, check out our example repository which uses the Rerun C++ SDK to log some data from Eigen and OpenCV: <https://github.com/rerun-io/cpp-example-opencv-eigen>.

## Using Rerun from CMake
Add the following to your `CMakeLists.txt`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL https://github.com/rerun-io/rerun/releases/download/prerelease/rerun_cpp_sdk.zip) # TODO(#3962): update link
FetchContent_MakeAvailable(rerun_sdk)
```

This will download a bundle with pre-built Rerun C static libraries for most desktop platforms, all Rerun C++ sources and headers, as well as CMake build instructions for them.

Currently, Rerun SDK works with C++17 or newer:
```cmake
set_property(TARGET example PROPERTY CXX_STANDARD 17)
```

Make sure you link with `rerun_sdk`:
```cmake
target_link_libraries(example PRIVATE rerun_sdk)
```

Combining the above, a minimal self-contained `CMakeLists.txt` looks like:
```cmake
project(example LANGUAGES CXX)
cmake_minimum_required(VERSION 3.16)

add_executable(example main.cpp)

# Download the rerun_sdk
include(FetchContent)
FetchContent_Declare(rerun_sdk URL https://github.com/rerun-io/rerun/releases/download/prerelease/rerun_cpp_sdk.zip) # TODO(#3962): update link
FetchContent_MakeAvailable(rerun_sdk)

# Rerun requires at least C++17, but it should be compatible with newer versions.
set_property(TARGET example PROPERTY CXX_STANDARD 17)

# Link against rerun_sdk.
target_link_libraries(example PRIVATE rerun_sdk)
```

## Logging some data
Add the following code to your `main.cpp`
<!-- TODO(#3962): Update Link -->
(This example also lives in the `rerun` source tree [example](https://github.com/rerun-io/rerun/blob/main/examples/cpp/minimal/main.cpp))
```cpp
#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using namespace rerun::demo;

int main() {
    // Create a new `RecordingStream` which sends data over TCP to the viewer process.
    auto rec = rerun::RecordingStream("rerun_example_cpp");
    rec.connect().throw_on_failure();

    // Create some data using the `grid` utility function.
    auto points = grid<rerun::Position3D, float>({-10.f, -10.f, -10.f}, {10.f, 10.f, 10.f}, 10);
    auto colors = grid<rerun::Color, uint8_t>({0, 0, 0}, {255, 255, 255}, 10);

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log("my_points", rerun::Points3D(points).with_colors(colors).with_radii({0.5f}));
}
```

Now start the viewer, build your application and run it:

You can configure cmake and build, for example like so:
```bash
cmake .
cmake --build . -j 8
rerun
./example
```

Once everything finishes compiling, you will see the points in the Rerun Viewer:

<picture>
  <img src="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/1200w.png">
</picture>

## Using the viewer
Try out the following to interact with the viewer:
 * Click and drag in the main view to rotate the cube.
 * Zoom in and out with the scroll wheel.
 * Mouse over the "?" icons to find out about more controls.
 * Click on the cube to select all of the points.
 * Hover and select individual points to see more information.

If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

## What's next

If you're ready to move on to more advanced topics, check out the [Viewer Walkthrough](viewer-walkthrough.md) or our
more advanced guide for [Logging Data in C++](logging-cpp.md) where we will explore the core concepts that make
Rerun tick and log our first non-trivial dataset.

If you'd rather learn from examples, check out the [example gallery](/examples) for some more realistic examples, or browse the [Types](../reference/types.md) section for more simple examples of how to use the main datatypes.
There's also a stand-alone example that shows [interop with Eigen and OpenCV](https://github.com/rerun-io/cpp-example-opencv-eigen).

TODO(#3977): Note that this is still an area of active development and there's going to be major improvements for library interop in upcoming versions.
