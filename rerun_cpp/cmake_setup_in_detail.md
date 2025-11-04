# CMake setup in detail

\tableofcontents

The Rerun C++ SDK is meant to be built from source and everything described on this page will do just that.
Its [CMake build script](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/CMakeLists.txt)
is ready to be used from outside of the Rerun repo.

## Download via FetchContent

By far the easiest way to add Rerun to your project is using `FetchContent`:
```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```

This will download a bundle with pre-built Rerun C static libraries for most desktop platforms,
all Rerun C++ sources and headers, as well as CMake build instructions for them.
By default this will also download & build [Apache Arrow](https://arrow.apache.org/)'s C++ library which is required to build the Rerun C++. See [Install Arrow C++](arrow_cpp_install.md) to learn more about this step and how to use an existing install.

We recommend this `FetchContent` workflow for all usecases since it is the easiest and works without any additional configuration.
All other workflows and configuration are there to best address more specific needs a project setup may haves.

## From Rerun repository

Alternatively, you can add the source of `https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/` directly to your own
project and then use `add_subdirectory`.

In this case you will also need to make sure the Rerun C static libraries are available for your target platforms.

Pre-built libraries can be downloaded from [the release pages](https://github.com/rerun-io/rerun/releases/latest).

If you want to match the behavior of `rerun_cpp_sdk.zip`, these libraries should be placed in the folder `rerun_cpp/lib`, renamed as:
 - Linux, x64: `librerun_c__linux_x64.a`
 - Linux, Arm64: `librerun_c__linux_arm64.a`
 - Windows, x64: `rerun_c__win_x64.lib`
 - Mac, Apple Silicon: `librerun_c__macos_arm64.a`

Or if you have a different build/download mechanism, you can point directly to the library by setting `RERUN_C_LIB`
before adding the subdirectory.

⚠️ Make sure **not** to add the root of the Rerun repository, as this will not only add many examples and tests
but also make additional assumptions about your build environment. For example it will always try to build
`rerun_c` (which the C++ SDK depends on) from its Rust source.

## CMake install

If you want to pre-build `rerun_sdk` for use with a different build system, or simply have a lot of projects using the same
`rerun_sdk`, it can be useful to use CMake's install command to install a re-usable version of `rerun_sdk` on your system.

To do so, follow these following steps:
* Download and unpack the desired `rerun_cpp_sdk.zip` (e.g https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip for the latest version)
* In the directory of the unpacked `rerun_cpp_sdk` run:
  * Configure:
    * `cmake -B build -S . -DCMAKE_BUILD_TYPE=Release`
  * Build:
    * `cmake --build build --config Release --target rerun_sdk`
  * Install:
    * `cmake --install build`
    * make sure you have permissions or use a target path, e.g. `--prefix ../rerun_sdk_install`
* Now that you have an install you can use `find_package(rerun_sdk REQUIRED)` in your project
  * Make sure that the prefix path or the rerun_sdk location is correctly configured.
  * Depending on your install path and OS this may work out of the box or require setting additional CMake variables (e.g. `-DCMAKE_PREFIX_PATH=rerun_sdk_install`)

The exact CMake invocations may need to be adjusted for your needs.

Naturally, you can also integrate `rerun_sdk`'s install into the install of your own libraries and executables.
This is generally only recommended for more advanced CMake setups.

As mentioned previously, by default Rerun's CMake script will download and build Arrow during its build.
Unless configured otherwise (see below) the resulting libraries are part of the `rerun_sdk` install.
⚠️ This does currently not work for dynamic Arrow libraries, i.e. if either one of
`RERUN_DOWNLOAD_AND_BUILD_ARROW=OFF` or `RERUN_ARROW_LINK_SHARED=ON` is set,
the install will use `find_package(Arrow)` to locate the Arrow library on your system.

# CMake configuration options

The C++ SDK provides a handful of configuration options.
All of them come with meaningful defaults, so typically you don't have to change any of them,
but they provide important hooks for more complex build setups.

Unless noted otherwise, a CMake install of `rerun_sdk` does **not** expose any of these options.

## RERUN_DOWNLOAD_AND_BUILD_ARROW
If enabled, will download a pinned version of the Apache Arrow C++ library and add it to the build.
Otherwise, `find_package` will be used to search for a pre-installed Arrow library.
For more information see the howto guide on [installing Arrow C++](arrow_cpp_install.md).

Defaults to `ON`.

## RERUN_ARROW_LINK_SHARED
If enabled, will use a dynamically linked version of Arrow, otherwise links statically with it.

Defaults to `OFF`.

Although enabling shared libraries makes linking faster and reduces binary size, it can present some challenges
related to locating the shared libraries at runtime. Depending on your system configuration it is even possible
to pick up a system-version of Arrow instead of the one you built against.

`rerun_sdk` installs that use a system installed Arrow library, can be configured using this option as well.

## RERUN_C_LIB
Path to the static Rerun C library to link against.

`rerun_c` is a static library built from a [Rust crate](https://github.com/rerun-io/rerun/tree/latest/crates/top/rerun_c).
It provides a minimalistic C interface that encapsulates the shared building blocks of all Rerun SDKs.

By default points to where a pre-built library for the currently active platform
is expected to be found in the Rerun C++ SDK distribution zip.

## RERUN_CPP_SOURCE_DIR
Path to the Rerun include and source directory, i.e. the directory that contains `rerun.hpp`.

Note that Rerun does not have separate folders for header (\*.hpp) and source (\*.cpp) files,
both are found inside `RERUN_CPP_SOURCE_DIR`.

By default is set to an absolute path that is determined by the location of Rerun's `CMakeLists.txt` itself.
Setting this is rarely needed, but reading it may be useful for build setups that can not rely on
the `rerun_cpp` target or for some reason aren't able to inherit the public target include path
set on `rerun_cpp`.
