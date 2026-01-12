---
title: C++
order: 1
---

## Setup

Before adding Rerun to your application, start by [installing the viewer](../../overview/installing-rerun/viewer.md).

## Learning by example

If you prefer to learn by example, check out our example repository which uses the Rerun C++ SDK to log some data from Eigen and OpenCV: <https://github.com/rerun-io/cpp-example-opencv-eigen>.

## Using Rerun with CMake

Assuming you are starting with a bare-bones `CMakeLists.txt` such as:

```cmake
cmake_minimum_required(VERSION 3.16...3.27)
project(example_minimal LANGUAGES CXX)

add_executable(example_minimal main.cpp)
```

You can add Rerun to your project using `FetchContent`

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```

This will download a bundle with pre-built Rerun C static libraries for most desktop platforms,
all Rerun C++ sources and headers, as well as CMake build instructions for them.
By default this will in turn download & build [Apache Arrow](https://arrow.apache.org/)'s C++ library which is required to build the Rerun C++.
See [Install Arrow C++](https://ref.rerun.io/docs/cpp/stable/md__2home_2runner_2work_2rerun_2rerun_2rerun__cpp_2arrow__cpp__install.html) to learn more about this step and how to use an existing install.

Finally, make sure you link with `rerun_sdk`:

```cmake
target_link_libraries(example_minimal PRIVATE rerun_sdk)
```

Combining the above, a minimal self-contained `CMakeLists.txt` looks like:

```cmake
cmake_minimum_required(VERSION 3.16...3.27)
project(example_minimal LANGUAGES CXX)

add_executable(example_minimal main.cpp)

# Download the rerun_sdk
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)

# Link against rerun_sdk.
target_link_libraries(example_minimal PRIVATE rerun_sdk)
```

Note that Rerun requires at least C++17. Depending on the sdk will automatically ensure that C++17 or newer is enabled.

## Logging some data

Add the following code to your `main.cpp`
(this example also lives in the `rerun` source tree [example](https://github.com/rerun-io/rerun/blob/latest/examples/cpp/minimal/main.cpp)):

```cpp
#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using namespace rerun::demo;

int main() {
    // Create a new `RecordingStream` which sends data over gRPC to the viewer process.
    const auto rec = rerun::RecordingStream("rerun_example_cpp");
    // Try to spawn a new viewer instance.
    rec.spawn().exit_on_failure();

    // Create some data using the `grid` utility function.
    std::vector<rerun::Position3D> points = grid3d<rerun::Position3D, float>(-10.f, 10.f, 10);
    std::vector<rerun::Color> colors = grid3d<rerun::Color, uint8_t>(0, 255, 10);

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log("my_points", rerun::Points3D(points).with_colors(colors).with_radii({0.5f}));
}
```

## Building and running

You can configure cmake, build, and run your application like so:

```bash
cmake -B build
cmake --build build -j
./build/example_minimal
```

Once everything finishes compiling, the application will spawn the Rerun Viewer and send the data to it:

<picture>
  <img src="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/intro_cpp_result/398c8fb79766e370a65b051b38eac680671c348a/1200w.png">
</picture>

## Using the Viewer

Try out the following to interact with the viewer:

-   Click and drag in the main view to rotate the cube.
-   Zoom in and out with the scroll wheel.
-   Mouse over the "?" icons to find out about more controls.
-   Click on the cube to select all of the points.
-   Hover and select individual points to see more information.

If you're facing any difficulties, don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) or [join the Discord server](https://discord.gg/PXtCgFBSmH).

## What's next

If you're ready to move on to more advanced topics, check out the [Viewer Walkthrough](../configure-the-viewer/navigating-the-viewer.md) or our
more advanced guide for [Logging Data in C++](../data-in/cpp.md) where we will explore the core concepts that make
Rerun tick and log our first non-trivial dataset.

If you'd rather learn from examples, check out the [example gallery](/examples) for some more realistic examples, or browse the [Types](../../reference/types.md) section for more simple examples of how to use the main datatypes.
There's also a stand-alone example that shows [interop with Eigen and OpenCV](https://github.com/rerun-io/cpp-example-opencv-eigen).

To learn more about how to work with your own types, check the [Custom Collection Adapter](https://github.com/rerun-io/rerun/tree/latest/examples/cpp/custom_collection_adapter) example on how to zero-copy adapt to Rerun types
and the [Use custom data](../../howto/logging-and-ingestion/custom-data.md) page for completely custom types.

To learn more about how to configure the C++ SDK's CMake file, check [CMake Setup in Detail](https://ref.rerun.io/docs/cpp/stable/md__2home_2runner_2work_2rerun_2rerun_2rerun__cpp_2cmake__setup__in__detail.html).
