---
title: C++ SDK CMake
order: 8
---

The Rerun C++ SDK is meant to be built from source.
Its [CMake build script](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/CMakeLists.txt)
is ready to be used from outside of the Rerun repo.

## Adding with FetchContent

The easiest way to add Rerun to your project is using `FetchContent`:
```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```

This will download a bundle with pre-built Rerun C static libraries for most desktop platforms,
all Rerun C++ sources and headers, as well as CMake build instructions for them.
By default this will also download & build [Apache Arrow](https://arrow.apache.org/)'s C++ library which is required to build the Rerun C++.  See [Install arrow-cpp](../howto/arrow-cpp-install.md) to learn more about this step and how to use an existing install.

## Adding via subdirectory

Alternatively, you can add the source of `https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/` directly to your own
project and then use `add_subdirectory`.

In this case you will also need to make sure the Rerun C static libraries are available for your target platforms.

Pre-built libraries can be downloaded from [the release pages](https://github.com/rerun-io/rerun/releases/latest).

If you want to match the behavior of `rerun_cpp_sdk.zip`, these libraries should be placed in the folder `rerun_cpp/lib`, renamed as:
 - Linux: `librerun_c__linux_x64.a`
 - Windows: `rerun_c__win_x64.lib`
 - Mac: `librerun_c__macos_x64.a`
 - Mac Arm: `librerun_c__macos_arm64.a`

Or if you have a different build/download mechanism, you can point directly to the library by setting `RERUN_C_LIB`
before adding the subdirectory.

⚠️ Make sure **not** to add the root of the Rerun repository, as this will not only add many examples and tests
but also make additional assumptions about your build environment. For example it will always try to build
`rerun_c` (which the C++ SDK depends on) from its Rust source.

## CMake configuration options

The C++ SDK provides a handful of configuration options.
All of them come with meaningful defaults, so typically you don't have to change any of them,
but they provide important hooks for more complex build setups.

### `RERUN_DOWNLOAD_AND_BUILD_ARROW`
If enabled, will download a pinned version of the Apache Arrow C++ library and add it to the build.
Otherwise, `find_package` will be used to search for a pre-installed Arrow library.
For more information see the howto guide on [installing arrow-cpp](../howto/arrow-cpp-install.md).

Defaults to `ON`.

### `RERUN_ARROW_LINK_SHARED`
If enabled, will use a dynamically linked version of Arrow, otherwise links statically with it.

Defaults to `OFF` on Windows and to `ON` on Linux and Mac.
This makes it a lot easier to relocate windows executable (don't need to copy Arrow.dll around!) which is less of a concern on Linux & Mac where .so/.dylib files found more easily.


### `RERUN_C_LIB`
Path to the static Rerun C library to link against.

`rerun_c` is a static library built from a [Rust crate](https://github.com/rerun-io/rerun/tree/latest/crates/rerun_c).
It provides a minimalistic C interface that encapsulates the shared building blocks of all Rerun SDKs.

By default points to where a pre-built library for the currently active platform
is expected to be found in the Rerun C++ SDK distribution zip.


## Tested compilers

The Rerun C++ SDK requires a C++17 compliant compiler.

As of writing we tested the SDK against:
* Apple Clang 14, 15
* GCC 9, 10, 12
* Visual Studio 2022
