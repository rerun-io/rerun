## C++ Quick Start

${SAFARI_WARNING}

#### Installing the Rerun viewer
The Rerun C++ SDK works by connecting to an awaiting Rerun Viewer over TCP.

Building from source:
* [Install cargo](https://rustup.rs/)
* `cargo install rerun-cli`

Using `pip`:
* `pip install rerun-sdk`

After you've installed it, type `rerun` in your terminal to start the viewer.


#### Using the Rerun C++ SDK
First install the `arrow-cpp` library on your system using your favorite package manager.

Then add the following to your `CMakeLists.txt`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL https://build.rerun.io/commit/06dd483/rerun_cpp_sdk.zip) # 2023-10-20
FetchContent_MakeAvailable(rerun_sdk)
```

Make sure you link with `rerun_sdk`.

##### Logging your own data

Put the following code to your `main.cpp`:

```rust
${EXAMPLE_CODE}
```

Start the rerun viewer (`rerun`) and then build and run your C++ program.

You should see the points in this viewer:

![Demo recording](https://static.rerun.io/intro_rust_result/cc780eb9bf014d8b1a68fac174b654931f92e14f/768w.png)

${HOW_DOES_IT_WORK}
