---
title: C++ SDK CMake
order: 8
---

The Rerun C++ SDK is meant to be built from source.
Its [CMake build script](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/CMakeLists.txt)
is ready to be used from outside of the Rerun repo: just add `https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/`
to your project via `add_subdirectory` or use `FetchContent` to use a pre-packed bundle that we provide with every release.

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
