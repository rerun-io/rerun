---
title: Set up a C++ project
order: 200
---

You should have already [installed the C++ SDK](../install-rerun/cpp.md).

We assume you have a working C++ toolchain and are using CMake to build your project.
For this project we will let Rerun download and build [Apache Arrow](https://arrow.apache.org/)'s C++ library itself.
To learn more about how Rerun's CMake script can be configured, see [CMake Setup in Detail](https://ref.rerun.io/docs/cpp/stable/md__2home_2runner_2work_2rerun_2rerun_2rerun__cpp_2cmake__setup__in__detail.html) in the C++ reference documentation.

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

You're now ready to follow the [Log and Ingest](../data-in.md) tutorial.
