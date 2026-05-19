# CLAUDE.md

Guidance for LLMs working in this repo.

## Project overview

Rerun: time-aware multimodal data stack + visualization for robotics, spatial AI, computer vision. SDKs (Python, Rust, C++) log rich data (images, point clouds, tensors, etc.). Viewer for visualization.

## Build system

`pixi` for task management + deps. See `pixi.toml` for full task list.

### Essential commands

**Building:**
- `pixi run py-build` - Build Python SDK into local .venv (uses uv)
- `pixi run rerun-build` - Build native viewer (without web viewer)
- `pixi run rerun-build-web` - Build web viewer (wasm)
- `pixi run cpp-build-all` - Build all C++ artifacts

**Running:**
- `pixi run rerun` - Run viewer
- `pixi run uvpy script.py` - Run Python scripts with rerun SDK
- `cargo run -p <package_name>` - Run specific Rust example (e.g., `cargo run -p dna`)

**Code generation:**
- `pixi run codegen` - Generate Rust/Python/C++ code from .fbs type definitions

**Formatting:**
- `pixi run rs-fmt` - Format Rust files. **Always run after editing Rust files, before committing.**
- `pixi run py-fmt` - Format Python files
- `pixi run cpp-fmt` - Format C++ files
- `pixi run toml-fmt` - Format TOML files

**Testing:**
- `cargo clippy -p <crate_name>` - Run rust checks before building
- `cargo nextest run --all-features --no-fail-fast -p <crate_name>` - Run tests for specific crate
  - Example: `cargo nextest run --all-features --no-fail-fast -p re_view_spatial`
- Use `cargo nextest` (not `cargo test`) for better output + parallelism
- Always use `--all-features` unless specific reason not to
- Use `--no-fail-fast` to gather all failures in single run

**Snapshots:**
- **`insta` snapshots**: Text-based, run with regular Rust tests. On failure: `cargo insta review` (install: `cargo install cargo-insta`)
- **Image comparison tests**: Render image vs checked-in reference. Uses `egui_kittest`'s `Harness::snapshot` + `TestContext` for mocking viewer.
  - Results saved to `tests/snapshots/`, failures produce `diff.png`
  - Update refs: `UPDATE_SNAPSHOTS=1`
  - Update from failed CI run: `./scripts/update_snapshots_from_ci.sh`
  - Best practices: see [egui_kittest README](https://github.com/emilk/egui/tree/master/crates/egui_kittest#snapshot-testing)

## Code generation system

**Critical: Never edit generated files directly.** All generated files marked "DO NOT EDIT" at top.

### Type definition flow

```
.fbs files (definitions/) → pixi run codegen → Generated code (Rust/Python/C++) + docs (docs/content/reference/types/)
```

- Type definitions in `crates/store/re_sdk_types/definitions/rerun/`
  - `datatypes/*.fbs` - Low-level types (Vec3D, Mat4x4, etc.)
  - `components/*.fbs` - Component types (Position3D, Color, etc.)
  - `archetypes/*.fbs` - Archetypes (Points3D, Image, etc.)
  - `blueprint/*.fbs` - Blueprint system types
- Codegen implementation in `crates/build/re_types_builder/`
- After modifying .fbs files, run `pixi run codegen` to regenerate

### Extension pattern

Add custom functionality to generated types via `_ext` files:
- Rust: `filename_ext.rs` (auto-imported by codegen)
- Python: `filename_ext.py` (mixed into generated class)
- C++: `filename_ext.cpp` (compiled + included auto, parts may be marked for copy into header by codegen)

## Code conventions

### General

- use `…` instead of `...` <!-- NOLINT -->
- Validate conventions via `pixi run lint-rerun <file>` (no file = check everything)

## Architecture overview

### Crate organization

```
crates/
├── build/     # Code generation (re_types_builder)
├── store/     # Data types, storage, querying
├── top/       # User-facing SDKs and CLI
└── viewer/    # Viewer UI and rendering
```

More details in `ARCHITECTURE.md`.

### Type system hierarchy

Three levels (generated from .fbs files):

1. **Datatypes** (`rerun.datatypes.*`) - Basic types like Vec3D, Color
2. **Components** (`rerun.components.*`) - Named semantic wrappers (Position3D, Radius)
3. **Archetypes** (`rerun.archetypes.*`) - Collections of components (Points3D, Image)

Each archetype specifies:
- Required components (must provide)
- Recommended components (good defaults)
- Optional components

Example: `Points3D` requires `positions`, recommends `colors` and `radii`, optional `labels`.

### Data flow

```
SDK (log archetype)
    ↓ encode to Apache Arrow
LogMsg (encoded data)
    ↓ transport (gRPC/file/memory)
re_chunk_store (indexed time series DB)
    ↓ query
Viewer (immediate mode rendering)
```

### Blueprint system

Viewer's configuration layer:
- Stored as separate store (`re_entity_db`) with "blueprint" timeline
- Defines: view layout, visibility, per-entity overrides, view properties
- Uses same type system as logged data
- Path hierarchy: `/viewport/`, `/view/{uuid}/`, `/container/{uuid}/`

### Visualizers

Each view type (Spatial3D, TimeSeries, etc.) has registered visualizers:
- Determine which entities/archetypes can be visualized
- Execute per-frame: query data → process → generate render commands
- Examples: Points3DVisualizer, LineStripsVisualizer, MeshVisualizer

Viewer uses **immediate mode**: every frame, query store + re-render from scratch.

## Documentation snippets

See [`docs/snippets/README.md`](docs/snippets/README.md) for running, building, finding snippets. Config in [`docs/snippets/snippets.toml`](docs/snippets/snippets.toml).

## Python development workflow

Python uses separate uv-managed .venv (not pixi's conda env):

```bash
pixi run py-build              # Build rerun-sdk into .venv
pixi run uvpy script.py        # Run Python scripts via uv
pixi run uv run script.py      # Explicit uv run
```

`uv` wrapper unsets `CONDA_PREFIX` for isolation from pixi's env.

## Important notes

- **PyO3 Configuration**: PyO3 config errors → run `pixi run ensure-pyo3-build-cfg`
- **git-lfs**: Required for test snapshots. Install + run `git lfs install`
- **Immediate Mode**: Entire viewer rendered from scratch each frame (no state management callbacks)
- **Arrow Native**: Data stored, transmitted, queried as Apache Arrow arrays
- **Multi-language**: .fbs changes affect Rust, Python, C++ simultaneously

## Python docstring formatting

Python API docs use **MkDocs + mkdocstrings** (NOT Sphinx). Never use reStructuredText (rST) in Python docstrings. Use markdown:

- Cross-refs: `[`ClassName`][]` not `:class:`ClassName`` / `:func:` / `:meth:`
- Warnings: `!!! warning` (MkDocs admonition with indented body) not `.. warning::`
- Deprecation: use `@deprecated` decorator (mkdocstrings renders it), don't duplicate in docstring
- Code blocks: markdown fenced blocks, not `.. code-block::`
- Params: numpy-style (`Parameters`, `Returns` with `----------`)

## Documentation system

See [`docs/README.md`](docs/README.md) for full docs architecture.

Docs span multiple sites: main docs at `rerun.io/docs` (from `docs/content/`), API refs for Python (MkDocs), C++ (Doxygen), JS (TypeDoc) at `ref.rerun.io/docs/{python,cpp,js}/`.

Key points:
- **`docs/content/reference/types/`** auto-generated by `pixi run codegen` from `.fbs` files - don't edit
- **`docs/content/reference/cli.md`** auto-generated by `pixi run man` - don't edit
- **Code snippets** in `docs/snippets/all/` with Python, Rust, C++ implementations
- `pixi run py-docs-serve` previews Python API docs locally
- `pixi run -e cpp cpp-docs` builds C++ docs

## Development references

- [`ARCHITECTURE.md`](ARCHITECTURE.md) - Detailed architecture docs
- [`BUILD.md`](BUILD.md) - Full build instructions
- [`CODE_STYLE.md`](CODE_STYLE.md) - Code style guidelines
- [`CONTRIBUTING.md`](CONTRIBUTING.md) - Contribution guidelines
- [`DESIGN.md`](DESIGN.md) - UI design guidelines (GUI, CLI, docs, log messages)
- [`docs/README.md`](docs/README.md) - Documentation system (sites, builds, deployment)
- [`rerun_py/README.md`](rerun_py/README.md) - Python SDK instructions
