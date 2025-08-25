# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## About Rerun

Rerun is a multimodal data stack and visualization platform for robotics, spatial AI, and computer vision. It provides SDKs for Python, Rust, and C++ to log time-aware data like images, point clouds, tensors, and text, with a native/web viewer for visualization and debugging.

## Essential Development Commands

### Building and Running
- **Run the viewer**: `pixi run rerun` (native) or `pixi run rerun-web` (web viewer)
- **Build release viewer**: `pixi run rerun-build-release`
- **Format code**: `pixi run format` (all languages) or `pixi run rs-fmt`/`pixi run py-fmt`/`pixi run cpp-fmt`
- **Run linter**: `pixi run rs-check`, `pixi run py-lint`, `pixi run lint-rerun`

### Python SDK Development
- **Build Python SDK**: `pixi run py-build` (debug) or `pixi run py-build-release`
- **Run Python tests**: `pixi run py-test`
- **Run examples**: `pixi run py-run-all-examples` (requires `pixi run py-build-examples` first)

### C++ SDK Development  
- **Build C++ SDK**: `pixi run -e cpp cpp-build-all`
- **Run C++ tests**: `pixi run -e cpp cpp-test`
- **Generate C++ docs**: `pixi run -e cpp cpp-docs`

### Rust Development
- **Test specific example**: `cargo run -p dna` (runs DNA helix example)
- **Build documentation**: `cargo doc --all-features --no-deps --open`
- **Run with web viewer**: `pixi run rerun-web-release`

### Testing Commands
- **Run all Rust tests**: `cargo test`
- **Test backwards compatibility**: `pixi run check-backwards-compatibility`
- **Update snapshot tests**: `pixi run rs-update-snapshot-tests`
- **Benchmark**: `pixi run py-bench` or `pixi run rs-plot-dashboard`

## High-Level Architecture

### Multi-Language SDK Structure
- **Rust Core** (`/crates/`): Foundation SDK with native viewer
- **Python SDK** (`/rerun_py/`): PyO3 bindings with maturin build system  
- **C++ SDK** (`/rerun_cpp/`): CMake-based API with doxygen documentation
- **JavaScript/Web** (`/rerun_js/`): Web viewer components and npm packages

### Core Technologies
- **Apache Arrow**: Columnar data storage/transport via `arrow` crate
- **Entity-Component Model**: Data organized by hierarchical entity paths and typed components
- **Time-Aware Storage**: Multiple timeline support (frame, wall-clock, custom) 
- **wgpu Graphics**: Cross-platform rendering (Vulkan/Metal/D3D12/WebGL/WebGPU)
- **egui GUI**: Immediate-mode cross-platform UI framework
- **WebAssembly**: Browser deployment via `wasm-bindgen`
- **gRPC/Protobuf**: Network transport via `tonic` and `prost`

### Key Data Concepts
- **Archetypes**: High-level logged data types (Points3D, Image, Transform3D, etc.) 
- **Components**: Individual data pieces that compose archetypes
- **Entity Paths**: Hierarchical organization (e.g., `/robot/camera/image`)
- **Chunks**: Apache Arrow-encoded batches of component data
- **Timelines**: Frame-based, wall-clock, or custom time indexing
- **`.rrd` files**: Binary log format (Rerun Data)

### Crate Organization
- **Store** (`crates/store/`): Data storage, querying, encoding (Arrow-based)
- **Viewer** (`crates/viewer/`): GUI components, rendering, visualization views
- **Utils** (`crates/utils/`): Common utilities, logging, error handling
- **Top** (`crates/top/`): SDK APIs and CLI binaries
- **Build** (`crates/build/`): Code generation and build tooling

## Development Practices

### Code Standards
- **No `unwrap`/`expect`**: Use proper error handling with `Result<T, E>`
- **No `unsafe`**: Minimize unsafe code, scrutinize during reviews
- **Error handling**: Use `re_log::error!`, `thiserror` for libraries, `anyhow` for applications
- **Avoid panics**: Code should never panic except for unrecoverable bugs
- **Iterator safety**: Sort collections when order matters, use `unsorted_` prefix if not

### Naming Conventions
- **Crates**: `snake_case` preferred over `kebab-case` 
- **Spaces**: Explicit naming like `world_from_view` for transforms
- **Units**: Explicit units in names (`duration_secs`, `distance_meters`)
- **UI functions**: `_ui` suffix for functions taking `&mut egui::Ui`

### Architecture Patterns
- **Immediate Mode**: GUI and rendering rebuilds each frame for responsiveness
- **Entity-Component-System**: Flexible data representation with composable components  
- **Time-Series Database**: In-memory Arrow-based storage with temporal queries
- **Multi-Timeline**: Support for multiple synchronized time axes

## Package Management

The project uses **Pixi** (conda-based) for reproducible cross-platform development:
- **Primary environments**: `default` (Rust), `py` (Python), `cpp` (C++), `examples`
- **Key features**: `wheel-build`, `python-dev`, `examples-common`
- **Configuration**: All dependencies defined in `pixi.toml`
- **Validation**: Run `pixi run check-env` to verify setup

## Testing Strategy

### Test Types
- **Unit tests**: Each crate has `tests/` directory for Rust unit tests
- **Python tests**: Located in `rerun_py/tests/unit/` with pytest
- **C++ tests**: CMake-based tests in `rerun_cpp/tests/`
- **Integration tests**: Cross-language roundtrip validation 
- **Snapshot tests**: Visual regression testing with insta
- **Roundtrip tests**: Verify data consistency across language boundaries

### Running Tests
- **Rust**: `cargo test` or `cargo test -p <crate_name>`
- **Python**: `pixi run py-test` 
- **C++**: `pixi run -e cpp cpp-test`
- **Examples**: `pixi run py-run-all-examples`

## Release and Deployment

- **Version management**: Centralized in workspace `Cargo.toml` as `0.25.0-alpha.1+dev`
- **Python wheels**: Built with `maturin` and `pixi run py-build-wheel` 
- **C++ packages**: CMake-based installation with package config
- **Web deployment**: Wasm compilation via `pixi run rerun-build-web-release`
- **Documentation**: Hosted on rerun.io with API references at ref.rerun.io

## Common Development Workflows

### Adding a New Component
1. Define in `crates/store/re_types/src/components/`
2. Register in component registry  
3. Update codegen if needed: `pixi run codegen`
4. Add tests and documentation
5. Update examples to demonstrate usage

### Working with Examples  
- **Location**: `examples/` with `rust/`, `python/`, `cpp/` subdirectories
- **Multi-language**: Most examples implemented in all three languages
- **Testing**: Examples are built and run in CI for validation
- **Adding**: Follow existing pattern and update `manifest.toml`

### Performance Optimization
- **Profiling**: Use `pixi run rerun-perf` for performance builds with telemetry
- **Debug vs Release**: Default dev profile has `opt-level = 1` for faster iteration
- **Immediate mode**: Optimize for per-frame performance since GUI rebuilds constantly
- **Memory management**: Use `re_memory` crate for tracking allocation patterns

This architecture supports Rerun's mission of making multimodal data debugging and visualization accessible across robotics, AI, and computer vision workflows.