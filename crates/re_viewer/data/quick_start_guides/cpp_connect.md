## C++ Quick Start

${SAFARI_WARNING}

#### Installing the Rerun viewer
The Rerun C++ SDK works by connecting to an awaiting Rerun Viewer over TCP.

Building from source:
* [Install cargo](https://rustup.rs/)
* `cargo install rerun-cli`

Using `pip`:
* `pip install rerun-sdk`

After you have installed it, type `rerun` in your terminal to start the viewer.


#### Using the Rerun C++ SDK
First you need to install the `arrow-cpp` library on your system using your favorite package manager.
If you are using [pixi], you can simply type `pixi global install arrow-cpp`.
Find more information at the official Arrow Apache [install guide](https://arrow.apache.org/install/).

If you're using CMake, you can add the following to your `CMakeLists.txt`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk DOWNLOAD_EXTRACT_TIMESTAMP ON URL https://github.com/rerun-io/rerun/releases/download/prerelease/rerun_cpp_sdk.zip) # TODO(#3962): update link
FetchContent_MakeAvailable(rerun_sdk)
```

This will download a bundle with pre-built Rerun C static libraries for most desktop platforms, all Rerun C++ sources and headers, as well as CMake build instructions for them.

Make sure you link with `rerun_sdk`:
```cmake
target_link_libraries(your_executable PRIVATE rerun_sdk)
```

##### Logging your own data

Put the following code to your `main.cpp`:

```cpp
${EXAMPLE_CODE}
```

Start the rerun viewer (`rerun`) and then build and run your C++ program.

You should see the points in this viewer:

![Demo recording](https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/768w.png)

${HOW_DOES_IT_WORK}
