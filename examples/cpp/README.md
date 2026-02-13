# Rerun C++ examples
The simplest example is [`minimal`](minimal/main.cpp). You may want to start there
using the accompanying [`C++ Quick Start`](https://www.rerun.io/docs/getting-started/data-in/cpp) guide.

## Build all examples
The CMake target `examples` is a convenient alias for building all CMake examples in one go.

You can use `pixi run -e cpp cpp-build-examples` to invoke it within the repository's Pixi environment.
After that, you can run individual examples from `./build/examples/cpp/` (e.g. `./build/examples/cpp/dna/example_dna`).

## Contributions welcome
Feel free to open a PR to add a new example!

See [`CONTRIBUTING.md`](../../CONTRIBUTING.md) for details on how to contribute.
