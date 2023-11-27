# Installing ArrowCpp separately

\tableofcontents

## Automatically download & build arrow from source (default)

By default, the Rerun C++ SDK's CMake script (which is part of the SDK's zip artifact that can be fetched via `FetchContent`)
will download a known compatible version of Arrow from GitHub and add it to the build.
The build configuration is kept to the minimum required by the Rerun C++ SDK.

To instead use an existing install of Arrow, disable the CMake build option `RERUN_DOWNLOAD_AND_BUILD_ARROW`.
(by passing `-DRERUN_DOWNLOAD_AND_BUILD_ARROW=OFF` to your CMake configure step).
This will cause Rerun to instead use CMake's `find_package` to look for a ready-to-use install of the Arrow C++ library.

For more information about CMake config options see [C++ SDK CMake](cmake_setup_in_detail.md).

## Install arrow-cpp with Pixi

[Pixi](https://prefix.dev/docs/pixi/overview) is a convenient tool for managing cross-platform project dependencies. In
fact, Rerun uses it for our own internal development dependency management, and you will find `pixi.toml` files in most
of our external examples.

Make sure to use `-DRERUN_DOWNLOAD_AND_BUILD_ARROW=OFF` when building, otherwise Rerun's CMake script
will download & build arrow instead, ignoring your Pixi install.
The advantage of using Pixi is that you can rely on pre-built artifacts rather than adding Arrows build to your own.
Also, Pixi is of course also useful for managing other dependencies like Eigen or OpenCV,
as well as fr pinning the version of your build tooling.

### Installing Pixi
On Mac or Linux you can just run:
```
curl -fsSL https://pixi.sh/install.sh | bash
```
Or on Windows:
```
iwr -useb https://pixi.sh/install.ps1 | iex
```

Alternatively if you are already a `cargo` user, you can install `pixi` via:
```
cargo install pixi
```

See the [Pixi installation guide](https://prefix.dev/docs/pixi/overview#installation) for other installation options.

### Adding Pixi to your own project

If you want to use `pixi` to manage dependencies in your own project, you can simply run `pixi init` in the root of your
project folder. This will create a `pixi.toml` file that manages the project. After that you can run
`pixi add arrow-cpp==10.0.1` to add arrow-cpp as a dependency to your project.

Now, any pixi tasks added to your project will have access to the `arrow-cpp` library.

Even without tasks, you can run `pixi shell` to create a shell environment where all your project dependencies
(including `arrow-cpp`) will be available. You can use this `pixi shell` to run you project's build commands.

Check out the [pixi docs](https://prefix.dev/docs/pixi/basic_usage) for more information on what you can do with pixi.

### Pixi in action

The rerun-cpp example: <https://github.com/rerun-io/cpp-example-opencv-eigen> ships with a `pixi.toml` file to manage
its dependencies, as well as a set of tasks to simplify running it.

If you have pixi installed, all you need to do to run the example is:
```
git clone https://github.com/rerun-io/cpp-example-opencv-eigen
cd cpp-example-opencv-eigen
pixi run example
```

### Known Issues

⚠️ [#4050](https://github.com/rerun-io/rerun/issues/4050) `arrow-cpp` needs to be held back to 10.0.1 to avoid conflicts
with the `rerun-sdk` package when installed in the same pixi environment.

⚠️ On Windows pixi only downloads release binaries which are **not** compatible with debug builds, causing runtime crashes.
For debug builds you have to build Arrow yourself, see [Building Arrow C++](https://arrow.apache.org/docs/developers/cpp/building.html)
or stick with `RERUN_DOWNLOAD_AND_BUILD_ARROW=ON`.

## Other ways to install arrow-cpp

Rerun will also work with any existing environment install of Arrow that works with `find_package`.
 - Arrow provides pre-built packages for many platforms.
   - See the list at: <https://arrow.apache.org/install/>
 - Conda-forge contains a package for Arrow:
   - <https://anaconda.org/conda-forge/arrow-cpp>
