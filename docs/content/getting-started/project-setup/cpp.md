---
title: Set up a C++ project
order: 200
description: Bootstrap a CMake project that links against the Rerun C++ SDK
---

You should have already [installed the C++ SDK](../install-rerun/cpp.md).

We assume you have a working C++ toolchain and are using CMake to build your project.
For this project we will let Rerun download and build [Apache Arrow](https://arrow.apache.org/)'s C++ library itself.
To learn more about how Rerun's CMake script can be configured, see [CMake setup in detail](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/cmake_setup_in_detail.md).

## Setting up your CMakeLists.txt

A minimal `CMakeLists.txt` looks like this:

```cmake
cmake_minimum_required(VERSION 3.16...3.27)
project(example_project LANGUAGES CXX)

add_executable(example_project main.cpp)

# Download the rerun_sdk
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)

# Link against rerun_sdk.
target_link_libraries(example_project PRIVATE rerun_sdk)
```

Note that Rerun requires at least C++17. Depending on the SDK will automatically ensure that C++17 or newer is enabled.

## Includes

To use Rerun all you need to include is `rerun.hpp`:

```cpp
#include <rerun.hpp>
```

## Building

```bash
cmake -B build
cmake --build build -j
./build/example_project
```

You're now ready to follow the [Log and Ingest](../../logging-and-ingestion/tutorial.md) tutorial.

## Build options and troubleshooting

### Arrow

By default, the SDK downloads and builds a pinned version of [Apache Arrow](https://arrow.apache.org/)'s C++ library (currently 18.0.0) as part of your build.
This needs internet access during the first build, and takes a while.

* To use an Arrow library that is already installed on your system, set `-DRERUN_DOWNLOAD_AND_BUILD_ARROW=OFF`.
  CMake then finds it with `find_package(Arrow)`.
  See [Install Arrow C++](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/arrow_cpp_install.md).
* To build Arrow from a local copy of the source archive (for example on a machine without internet access), set `-DRERUN_ARROW_URL=/path/to/apache-arrow-18.0.0.tar.gz`.
  The archive must match the pinned version, because its checksum is verified.

### CMake 4

Arrow 18 does not support CMake 4 by itself.
The SDK works around this by passing `CMAKE_POLICY_VERSION_MINIMUM=3.5` to the Arrow build when it detects CMake 4.
If you need a different value, set `CMAKE_POLICY_VERSION_MINIMUM` yourself, and the SDK passes that on instead.

### Other platforms and cross-compiling

The SDK bundle contains prebuilt `rerun_c` libraries for Linux (x86_64 and arm64), macOS (arm64), and Windows (x86_64).
For any other platform, including when cross-compiling, build `rerun_c` yourself from the [Rerun repository](https://github.com/rerun-io/rerun) and point CMake to it:

```bash
cargo build --release -p rerun_c --target <target-triple>
cmake -B build -DRERUN_C_LIB=/path/to/librerun_c.a
```

See [CMake setup in detail](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/cmake_setup_in_detail.md) for all CMake options.
