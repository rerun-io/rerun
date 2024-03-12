# Rerun changelog

## [Unreleased](https://github.com/rerun-io/rerun/compare/latest...HEAD)

## [0.14.1](https://github.com/rerun-io/rerun/compare/0.14.0...0.14.1) - C++ build artifact fix

This release is identical to 0.14.0 and merely fixes an issue in the build artifacts for C++:
0.14.0 only contained binaries for Linux x64, this release has the full set for Linux x64, Windows x64, Mac x64 & Mac Arm64.

## [0.14.0](https://github.com/rerun-io/rerun/compare/0.13.0...0.14.0) - "Unlimited" point clouds & lines, quality of life improvements, bugfixes

### Overview & Highlights

Originally, we planned to do only a bugfix release, but we got an unexpected amount of goodies amassed already.
We're still ramping up for programmable blueprints (soon!), but meanwhile enjoy these improvements in 0.14!

- 📈 Limits for number of points & lines per space view lifted.
- 🖱️ Added context menu (right-click) actions for items on the Blueprint panel. (Only getting started on this, more actions in future releases!)
- 🚀 Speed improvements for scenes with many transforms and large point clouds.
- 🔺 Built-in STL mesh support.
- 🎥 First-person camera.
- 🐛 Fixes regressions in Space View spawn heuristics from 0.13, and many more bugfixes.
- 🧑‍🏫 Two new examples: [Gesture Recognition](https://github.com/rerun-io/rerun/tree/release-0.14.0/examples/python/gesture_detection) & [RRT* Pathfinding](https://github.com/rerun-io/rerun/tree/release-0.14.0/examples/python/rrt-star)

### Details

#### 🪵 Log API
- Add helpers for perspective cameras [#5238](https://github.com/rerun-io/rerun/pull/5238)
- Fix `spawn` starting the viewer even if logging is disabled [#5284](https://github.com/rerun-io/rerun/pull/5284)

#### 🐍 Python API
- Add missing python docs for `disable_timeline` & `reset_time` [#5269](https://github.com/rerun-io/rerun/pull/5269)
- Fix missing error message when passing `from_parent` + rerun transform type to `rerun.Transform3D` [#5270](https://github.com/rerun-io/rerun/pull/5270)

#### 🦀 Rust API
- Fix using `rerun` crate as a dependency on CI [#5170](https://github.com/rerun-io/rerun/pull/5170)

#### 🪳 Bug Fixes
- Enforce the rule: heuristics should never add a new view that would be completely covered by an existing view [#5164](https://github.com/rerun-io/rerun/pull/5164)
- Remove log spam when quickly resizing the viewer [#5189](https://github.com/rerun-io/rerun/pull/5189)
- Fix incorrect minimum supported rust version mentioned in docs and examples [#5195](https://github.com/rerun-io/rerun/pull/5195)
- Less restrictive visualizability constraints of 2D entities, improved space view generation heuristics [#5188](https://github.com/rerun-io/rerun/pull/5188)
- Fix ugly UI for some arrow data [#5235](https://github.com/rerun-io/rerun/pull/5235)
- Fix missing redraw upon resetting blueprint [#5262](https://github.com/rerun-io/rerun/pull/5262)
- Fix non-deterministic redundancy check for space view spawning heuristic [#5266](https://github.com/rerun-io/rerun/pull/5266)
- Fix resetting vertical axis when using non-uniform zoom on Time Series [#5287](https://github.com/rerun-io/rerun/pull/5287)

#### 🌁 Viewer Improvements
- Clear all blueprints in RAM and on disk when clicking "Reset Viewer" [#5199](https://github.com/rerun-io/rerun/pull/5199)
- Improve the orbit eye to always maintain an up-axis [#5193](https://github.com/rerun-io/rerun/pull/5193)
- Focus on current bounding-box when resetting camera-eye on a 3D space view (double click it) [#5209](https://github.com/rerun-io/rerun/pull/5209)
- Add STL mesh support [#5244](https://github.com/rerun-io/rerun/pull/5244)
- Add first person 3D eye-camera [#5249](https://github.com/rerun-io/rerun/pull/5249)

#### 🚀 Performance Improvements
- More robust handling of maximum texture size for non-color data, slight perf improvements for large point clouds [#5229](https://github.com/rerun-io/rerun/pull/5229)
- Cached transforms & disconnected spaces for faster scenes with many transforms [#5221](https://github.com/rerun-io/rerun/pull/5221)
- Optimized cpu time for 3D point clouds (once again!) [#5273](https://github.com/rerun-io/rerun/pull/5273)
- Only compute store/caching stats when the memory panel is opened [#5274](https://github.com/rerun-io/rerun/pull/5274)
- Increase the max WebSocket frame limit for the native client [#5282](https://github.com/rerun-io/rerun/pull/5282)

#### 🧑‍🏫 Examples
- Add Gesture Recognition example [#5241](https://github.com/rerun-io/rerun/pull/5241) (thanks [@andreasnaoum](https://github.com/andreasnaoum)!)
- Add example visualizing RRT* [#5214](https://github.com/rerun-io/rerun/pull/5214) (thanks [@02alexander](https://github.com/02alexander)!)

#### 📚 Docs
- Fix broken link in the installing-viewer documentation [#5236](https://github.com/rerun-io/rerun/pull/5236) (thanks [@BirgerMoell](https://github.com/BirgerMoell)!)

#### 🖼 UI Improvements
- Context Menu 1: Basic scaffolding and simple actions [#5163](https://github.com/rerun-io/rerun/pull/5163)
- Context menu 2: add support for multiple selection [#5205](https://github.com/rerun-io/rerun/pull/5205)
- Context menu 3: add "Move to new container" context menu action [#5210](https://github.com/rerun-io/rerun/pull/5210)
- Context menu 4: add "Clone space view" action [#5265](https://github.com/rerun-io/rerun/pull/5265)
- Context menu 5: refactor into multiple files [#5289](https://github.com/rerun-io/rerun/pull/5289)
- Clickable path parts in selection-panel [#5220](https://github.com/rerun-io/rerun/pull/5220)
- Don't show the blueprint section when selecting recordings [#5245](https://github.com/rerun-io/rerun/pull/5245)
- Use the same icon for recordings everywhere [#5246](https://github.com/rerun-io/rerun/pull/5246)

#### 🎨 Renderer Improvements
- Lift point cloud size limitations [#5192](https://github.com/rerun-io/rerun/pull/5192)
- Lift line vertex/strip count limitations [#5207](https://github.com/rerun-io/rerun/pull/5207)
- Fix banding artifacts of 3D space view's skybox [#5279](https://github.com/rerun-io/rerun/pull/5279)

#### 📦 Dependencies
- Bump maturin to 1.14.0 [#5197](https://github.com/rerun-io/rerun/pull/5197)
- Update `tungstenite` to remove RUSTSEC warning [#5200](https://github.com/rerun-io/rerun/pull/5200)
- Lock the web-sys version to 0.3.67 [#5211](https://github.com/rerun-io/rerun/pull/5211)


## [0.13.0](https://github.com/rerun-io/rerun/compare/0.12.1...0.13.0) - Fast time series, improved layout editing & UI overrides - 2024-02-12

### Overview & Highlights

This release focuses on scalar time series -- both from a performance and UI perspectives.
Check out our [associated blog post](https://www.rerun.io/blog/fast-plots) for more information.

- 📈 Rerun can now visualize many time series in the kHz range in real-time:
    - The new query cache optimizes data access, improving query performance by 20-50x
    - Sub-pixel aggregation prevents unnecessary overdraw when rendering plots, improving rendering time by 30-120x
    - [Points](https://www.rerun.io/docs/reference/types/archetypes/points3d), [lines](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d), [arrows](https://www.rerun.io/docs/reference/types/archetypes/arrows3d) and [boxes](https://www.rerun.io/docs/reference/types/archetypes/boxes3d) all benefit from query caching too to a lesser extent, yielding 2-5x performance improvements

- 🖼 UI overrides:
    - The new `Scalar`, `SeriesLine` & `SeriesPoint` archetypes allow for customizing plots both at logging and visualization time
    - Customize marker shapes, marker sizes, etc from code or directly through the UI
    - Specify axis labels, lock axes, etc from code or directly through the UI

- 🌁 Viewer:
    - The number of compute threads can now be controlled using the `--threads`/`-j` flag
    - Added support YUY2-encoded images (thanks [@oxkitsune](https://github.com/oxkitsune)!)
    - Space views can now be drag-and-dropped directly from the blueprint tree
    - Scenes with 100+ entities are now up to 5x faster.

- 🚚 New Space View and Container creation workflow:
    - When selected, containers have a children list in the Selection Panel, where new Space Views and Containers may be added.
    - New modal dialog to add Space Views and Containers.
    - The same dialog is also available from the `+` button of the Blueprint tree UI.
    - The Space View's origin can now be edited in the Selection Panel.
    - The container hierarchy can now be cleaned up with the new `Simplify Hierarchy` button in the Selection Panel for containers.

- 🦀 The rust SDK now exposes an optional integration with the `mint` crate
- 🕸️ The web UI SDK now supports loading multiple `.rrd` URLs
- 🔺 The web viewer now renders using WebGPU by default (when available), leading to lower memory usage on Chrome.
  You can override this behavior using `?renderer=webgl`/`?renderer=webgpu` url parameter, or restart with WebGL/WebGPU respectively from the options menu.

As well as a lot of miscellaneous bug fixes and usability improvements: see details below.

Check out our [migration guide](https://www.rerun.io/docs/reference/migration/migration-0-13).

### Details

#### 🪵 Log API
- Mark TimeSeriesScalar as deprecated in all SDKs and documentation [#5102](https://github.com/rerun-io/rerun/pull/5102)

#### 🌊 C++ API
- Document that in C++ PinholeProjection::from_mat3x3 is column major [#4843](https://github.com/rerun-io/rerun/pull/4843)
- Include LICENSE files into C++ SDK Assets [#4870](https://github.com/rerun-io/rerun/pull/4870) (thanks [@rgolovanov](https://github.com/rgolovanov)!)
- Fix C++ arrow build flag forwarding [#4921](https://github.com/rerun-io/rerun/pull/4921) (thanks [@rgolovanov](https://github.com/rgolovanov)!)

#### 🦀 Rust API
- Add integration with the `mint` crate [#4753](https://github.com/rerun-io/rerun/pull/4753)

#### 🐍 Python API
- Fix support for compressing mono images by respecting mode to determine depth [#4847](https://github.com/rerun-io/rerun/pull/4847)

#### 🪳 Bug Fixes
- External loader: don't do process IO on compute thread-pool [#4942](https://github.com/rerun-io/rerun/pull/4942)
- Fix a Visible Time Range UI issue where the summary string would display the wrong data range [#5034](https://github.com/rerun-io/rerun/pull/5034)
- Clear empty containers after tile drag-and-drop [#5044](https://github.com/rerun-io/rerun/pull/5044)
- Allow for very large meshes & plots by always picking the largest available GPU buffer size [#5053](https://github.com/rerun-io/rerun/pull/5053)
- Fix forever repaint of big scenes [#5071](https://github.com/rerun-io/rerun/pull/5071)
- Fix `RERUN_FLUSH_NUM_BYTES` and data size estimations [#5086](https://github.com/rerun-io/rerun/pull/5086)
- Make `rectangle_fs.wgsl` compile on chrome despite angle/mesa bug (#3931) [#5074](https://github.com/rerun-io/rerun/pull/5074)

#### 🌁 Viewer Improvements
- Introduce Scalar, SeriesLine, and SeriesPoint archetypes with their own visualizers [#4875](https://github.com/rerun-io/rerun/pull/4875)
- Support modifying the plot style by introducing a generic framework for overriding components [#4914](https://github.com/rerun-io/rerun/pull/4914)
- Introduce a new blueprint archetype for AxisY configuration in a plot [#5028](https://github.com/rerun-io/rerun/pull/5028)
- Improve the selection/hover behavior for plots [#5096](https://github.com/rerun-io/rerun/pull/5096)
- Click a spatial space view background to select the space view itself [#4796](https://github.com/rerun-io/rerun/pull/4796)
- Double-clicking an entity in the blueprint & time panels focuses the 3D camera on it [#4799](https://github.com/rerun-io/rerun/pull/4799)
- When loading a .ply file, warn about ignored properties [#4934](https://github.com/rerun-io/rerun/pull/4934)
- Make it easier to position 3D eye-camera center [#4943](https://github.com/rerun-io/rerun/pull/4943)
- Include tessellation and rendering in CPU time shown in top bar [#4951](https://github.com/rerun-io/rerun/pull/4951)
- Allow selection of entities directly in the plot space view [#4959](https://github.com/rerun-io/rerun/pull/4959)
- Texture support for raw `Mesh3D` logging [#4894](https://github.com/rerun-io/rerun/pull/4894)

#### 🚀 Performance Improvements
- Add `--threads` / `-j` to control number of compute threads [#5021](https://github.com/rerun-io/rerun/pull/5021)
- Introduce the query cache:
    - Primary caching 3: bare-bone latest-at caching [#4659](https://github.com/rerun-io/rerun/pull/4659)
    - Primary caching 4: runtime toggle support [#4680](https://github.com/rerun-io/rerun/pull/4680)
    - Primary caching 5: 2D & 3D point clouds [#4681](https://github.com/rerun-io/rerun/pull/4681)
    - Primary caching 6: TextLogs & TimeSeries [#4698](https://github.com/rerun-io/rerun/pull/4698)
    - Primary caching 7: Always expose the data time in query responses [#4711](https://github.com/rerun-io/rerun/pull/4711)
    - Primary caching 8: implement latest-at data-time cache entry deduplication [#4712](https://github.com/rerun-io/rerun/pull/4712)
    - Primary caching 9: timeless latest-at support [#4721](https://github.com/rerun-io/rerun/pull/4721)
    - Primary caching 10: latest-at cache invalidation [#4726](https://github.com/rerun-io/rerun/pull/4726)
    - Primary caching 11: cache stats and integration with memory panel [#4773](https://github.com/rerun-io/rerun/pull/4773)
    - Primary caching 12: bare-bone range support [#4784](https://github.com/rerun-io/rerun/pull/4784)
    - Primary caching 13: stats & memory panel integration for range queries [#4785](https://github.com/rerun-io/rerun/pull/4785)
    - Primary caching 14: don't bake `LatestAt(T-1)` results into low-level range queries [#4793](https://github.com/rerun-io/rerun/pull/4793)
    - Primary caching 15: range read performance optimization [#4800](https://github.com/rerun-io/rerun/pull/4800)
    - Primary caching 16: context-free range semantics [#4851](https://github.com/rerun-io/rerun/pull/4851)
    - Primary caching 17: timeless range [#4852](https://github.com/rerun-io/rerun/pull/4852)
    - Primary caching 18: range invalidation (ENABLED BY DEFAULT :confetti_ball:) [#4853](https://github.com/rerun-io/rerun/pull/4853)
    - Primary caching 19 (final): make cache globals non-static [#4856](https://github.com/rerun-io/rerun/pull/4856)
- Integrate query caching with more primitives:
    - Cached 2D & 3D box clouds [#5000](https://github.com/rerun-io/rerun/pull/5000)
    - Cached 2D & 3D line clouds [#5083](https://github.com/rerun-io/rerun/pull/5083)
    - Cached 2D & 3D arrow clouds [#5088](https://github.com/rerun-io/rerun/pull/5088)
- Configurable dynamic plot aggregation based on zoom-level [#4865](https://github.com/rerun-io/rerun/pull/4865)
- Improved automatic view creation heuristic, major speedup for scenes with many entities [#4874](https://github.com/rerun-io/rerun/pull/4874)
- Optimize point clouds [#4932](https://github.com/rerun-io/rerun/pull/4932)

#### 🧑‍🏫 Examples
- Update all examples that use `TimeSeriesScalar` to use `Scalar` instead [#5042](https://github.com/rerun-io/rerun/pull/5042)

#### 📚 Docs
- Improve documentation of the `Clear` archetype [#4760](https://github.com/rerun-io/rerun/pull/4760)
- `DisconnectedSpace` now only applies to spatial space views [#4935](https://github.com/rerun-io/rerun/pull/4935)
- Fill gaps in image encoding documentation, fix how python documents union variants [#4988](https://github.com/rerun-io/rerun/pull/4988)

#### 🖼 UI Improvements
- Improve timeseries Space Views:
  - Introduce a new component for MarkerShape and use it in SeriesPoint [#5004](https://github.com/rerun-io/rerun/pull/5004)
  - Introduce a new StrokeWidth component and use it for SeriesLine [#5025](https://github.com/rerun-io/rerun/pull/5025)
  - Break up plot charts when there's a `Clear` [#4957](https://github.com/rerun-io/rerun/pull/4957)
  - Only show the LegacyVisualizer if a user logs with TimeSeriesScalar archetype [#5023](https://github.com/rerun-io/rerun/pull/5023)
  - Fix lagging time cursor when panning a time series plot [#4972](https://github.com/rerun-io/rerun/pull/4972)
- New Space View and Container creation workflow:
  - Use the "Add Space View/Container" modal for the `+` button of the blueprint tree [#5012](https://github.com/rerun-io/rerun/pull/5012)
  - Add support for removing container children from the selection panel [#4930](https://github.com/rerun-io/rerun/pull/4930)
  - Add support for full span highlighting to modal and use it in the "Add Space View or Container" modal [#4822](https://github.com/rerun-io/rerun/pull/4822)
  - Remove the "+" icon from the "Add SV/Container" modal and close on click [#4927](https://github.com/rerun-io/rerun/pull/4927)
  - New empty space view defaults to uncollapsed in blueprint tree [#4982](https://github.com/rerun-io/rerun/pull/4982)
  - Do not allow adding Horizontal/Vertical containers inside of containers with the same type [#5091](https://github.com/rerun-io/rerun/pull/5091)
- Selection improvements:
  - Click a recording to select it [#4761](https://github.com/rerun-io/rerun/pull/4761)
  - Press the escape key to clear the current selection [#5103](https://github.com/rerun-io/rerun/pull/5103)
  - Clear selection when clicking blank space in the Blueprint View [#4831](https://github.com/rerun-io/rerun/pull/4831)
  - Selecting/hovering components now highlights their parent entity [#4748](https://github.com/rerun-io/rerun/pull/4748)
- Add support for drag-and-drop in blueprint tree [#4910](https://github.com/rerun-io/rerun/pull/4910)
- Add support for editing a space view's space origin [#4848](https://github.com/rerun-io/rerun/pull/4848)
- Add Help and Discord to command palette [#4752](https://github.com/rerun-io/rerun/pull/4752)
- Syntax highlighting of entity paths and instance paths [#4803](https://github.com/rerun-io/rerun/pull/4803)
- Update container (and a couple other) icons [#4814](https://github.com/rerun-io/rerun/pull/4814)
- Make space view names optional and subdue placeholder view label in the UI [#4682](https://github.com/rerun-io/rerun/pull/4682)
- Show download sizes of in the example page [#4841](https://github.com/rerun-io/rerun/pull/4841)
- Style container's label as unnamed [#4975](https://github.com/rerun-io/rerun/pull/4975)
- Fix space view cloning to also copy entity properties (visible time range, etc.) [#4978](https://github.com/rerun-io/rerun/pull/4978)
- Improve how the root container is displayed and handled in the blueprint tree [#4989](https://github.com/rerun-io/rerun/pull/4989)
- Improve the UI for the entity query [#5022](https://github.com/rerun-io/rerun/pull/5022)
- Don't show the Blueprint header when on the welcome screen [#5046](https://github.com/rerun-io/rerun/pull/5046)
- Move Visible Time Range higher in the Selection Panel [#5036](https://github.com/rerun-io/rerun/pull/5036)
- Clean up time range ui [#5089](https://github.com/rerun-io/rerun/pull/5089)
- Improve preview UI for Component data [#5093](https://github.com/rerun-io/rerun/pull/5093)
- Paint closest labels on top of labels further away [#5124](https://github.com/rerun-io/rerun/pull/5124)

#### 🕸️ Web
- Web: Support multiple `.rrd` URLs [#4740](https://github.com/rerun-io/rerun/pull/4740)
- Unify `web_viewer/index.html` and `index_bundled.html` [#4720](https://github.com/rerun-io/rerun/pull/4720)
- Allow forcing WebGPU/WebGL on the web player, new command line argument to force graphics backend [#4981](https://github.com/rerun-io/rerun/pull/4981)

#### 🎨 Renderer Improvements
- Update to wgpu 0.19 and latest `egui` trunk [#4885](https://github.com/rerun-io/rerun/pull/4885)
- Support YUY2-encoded images [#4877](https://github.com/rerun-io/rerun/pull/4877) (thanks [@oxkitsune](https://github.com/oxkitsune)!)

#### 🧑‍💻 Dev-experience
- Default to DEBUG log level in debug builds [#4749](https://github.com/rerun-io/rerun/pull/4749)
- New debug option to show an actual timeline for the Blueprint [#4609](https://github.com/rerun-io/rerun/pull/4609)
- Primary cache: basic debug tools via command palette [#4948](https://github.com/rerun-io/rerun/pull/4948)

#### 🗣 Refactors
- Migrate from `egui_Tile::TileId` to proper blueprint IDs in `ViewportBlueprint` API [#4900](https://github.com/rerun-io/rerun/pull/4900)

#### 📦 Dependencies
- Remove `egui_plot` as dependency from `re_sdk` [#5099](https://github.com/rerun-io/rerun/pull/5099)
- Update to egui 0.25 and winit 0.29 [#4732](https://github.com/rerun-io/rerun/pull/4732)
- Prune dependencies from `rerun` and `re_sdk` [#4824](https://github.com/rerun-io/rerun/pull/4824)
- Relax pyarrow dependency to `>=14.0.2` [#5054](https://github.com/rerun-io/rerun/pull/5054)
- Update egui_tiles to 0.7.2 [#5107](https://github.com/rerun-io/rerun/pull/5107)

#### 🤷‍♂️ Other
- Add `rerun --serve` and improve `--help` [#4834](https://github.com/rerun-io/rerun/pull/4834)
- `rerun print`: print just summary, unless given `--verbose` [#5079](https://github.com/rerun-io/rerun/pull/5079)


## [0.12.1](https://github.com/rerun-io/rerun/compare/0.12.0...0.12.1) - 2024-01-17 - Data loader bug fixes

#### 🌊 C++ API
- Fix CMake trying to pick up test folders outside of the Rerun project/zip [#4770](https://github.com/rerun-io/rerun/pull/4770) (thanks [@KevinGliewe](https://github.com/KevinGliewe)!)
- Document that `Mat3x3` and `Mat4x4` constructors are column major [#4842](https://github.com/rerun-io/rerun/pull/4842)

#### 🦀 Rust API
- Fix `entity_path_vec!` and `entity_path!` depending on `ToString` being in scope [#4766](https://github.com/rerun-io/rerun/pull/4766) (thanks [@kpreid](https://github.com/kpreid)!)

#### 🪳 Bug Fixes
- Fix external data loader plugins on Windows [#4840](https://github.com/rerun-io/rerun/pull/4840)
- Reduce latency when loading data from external loaders [#4797](https://github.com/rerun-io/rerun/pull/4797)
- Always point to versioned manifest when building a versioned binary [#4781](https://github.com/rerun-io/rerun/pull/4781)

#### 🧑‍💻 Dev-experience
- External loaders: remove warnings on duplicated binary on `$PATH` [#4833](https://github.com/rerun-io/rerun/pull/4833)

#### 🤷‍♂️ Other
- Include `Cargo.lock` in `rerun-cli` crate [#4750](https://github.com/rerun-io/rerun/pull/4750)
- Replace `atty` dependency with `std::io::IsTerminal` [#4790](https://github.com/rerun-io/rerun/pull/4790) (thanks [@kpreid](https://github.com/kpreid)!)


## [0.12.0](https://github.com/rerun-io/rerun/compare/0.11.0...0.12.0) - Data Loaders, Container-editing, Python-3.12 - 2024-01-09

### Overview & Highlights
- 🌁 The Rerun Viewer now supports a plugin system for creating [arbitrary external data loaders](https://www.rerun.io/docs/howto/open-any-file).
- 🕸️ More built-in examples are now available in the viewer.
- 🐍 The Python SDK now works with Python-3.12.
- 📘 Blueprint containers can now be selected and modified.
- 🚀 In the native viewer, space views are now evaluated in parallel for improved performance.
- 🧑‍🏫 Support and guide for [sharing a recording across multiple processes](https://www.rerun.io/docs/howto/shared-recordings).
- 📁 Entity-paths allowed characters and escaping are now more file-like [#4476](https://github.com/rerun-io/rerun/pull/4476):
 - There is no need for " quotes around path parts, instead we now use \ to escape special characters.
 - You need to escape any character that isn't alphabetical, numeric, ., -, or _.

### Details

#### 🌊 C++ API
- Exposing `recording_id` in C and C++ SDKs [#4384](https://github.com/rerun-io/rerun/pull/4384)
- All C++ preprocessor macros start now with RR_ (instead of a mix of RR_ and RERUN_) [#4371](https://github.com/rerun-io/rerun/pull/4371)
- C++ & Python API: add helpers for constructing an entity path [#4595](https://github.com/rerun-io/rerun/pull/4595)

#### 🐍 Python API
- Add `--stdout`/`-o` to our CLI helper library [#4544](https://github.com/rerun-io/rerun/pull/4544)
- C++ & Python API: add helpers for constructing an entity path [#4595](https://github.com/rerun-io/rerun/pull/4595)
- Python SDK: introduce deferred garbage collection queue [#4583](https://github.com/rerun-io/rerun/pull/4583)
- Add support for Python 3.12 [#4146](https://github.com/rerun-io/rerun/pull/4146)

#### 🦀 Rust API
- Exposing `recording_id` in Rust SDK [#4383](https://github.com/rerun-io/rerun/pull/4383)
- Add `--stdout`/`-o` to our CLI helper library [#4544](https://github.com/rerun-io/rerun/pull/4544)
- Document how to construct an entity path for the Rust logging API [#4584](https://github.com/rerun-io/rerun/pull/4584)

#### 🪳 Bug Fixes
- Bugfix: show labels on segmentation images with trivial dimensions [#4368](https://github.com/rerun-io/rerun/pull/4368)
- Datastore: don't eagerly sort in bucket split routine on ingestion path [#4417](https://github.com/rerun-io/rerun/pull/4417)
- Resolve spurious blueprint panel group collapsing [#4548](https://github.com/rerun-io/rerun/pull/4548)
- Fix rectangle that indicates the zoomed pixel area on hover being one pixel to small [#4590](https://github.com/rerun-io/rerun/pull/4590)
- Fix wrong RowId order of logged data [#4658](https://github.com/rerun-io/rerun/pull/4658)
- Make scroll-to-zoom a lot more responsive in 3D views [#4668](https://github.com/rerun-io/rerun/pull/4668)
- Fix heuristic object properties being broken in some cases / fix DepthMeter being ignored sometimes [#4679](https://github.com/rerun-io/rerun/pull/4679)

#### 🌁 Viewer Improvements
- Make viewer contexts's render context reference non-mutable [#4430](https://github.com/rerun-io/rerun/pull/4430)
- The Rerun Viewer can now consume from stdin:
  - Standard input/output support 1: stream RRD data from stdin [#4511](https://github.com/rerun-io/rerun/pull/4511)
  - Standard input/output support 2: Rust SDK stdout impl/examples/docs [#4512](https://github.com/rerun-io/rerun/pull/4512)
  - Standard input/output support 3: Python SDK stdout impl/examples/docs [#4513](https://github.com/rerun-io/rerun/pull/4513)
  - Standard input/output support 4: C++ SDK stdout impl/examples/docs [#4514](https://github.com/rerun-io/rerun/pull/4514)
- Support for custom DataLoaders:
  - `DataLoader`s 0: utility for hierarchical `EntityPath` from file path [#4516](https://github.com/rerun-io/rerun/pull/4516)
  - `DataLoader`s 1: introduce, and migrate to, `DataLoader`s [#4517](https://github.com/rerun-io/rerun/pull/4517)
  - `DataLoader`s 2: add text-based `DataLoader` (`.txt`, `.md`) [#4518](https://github.com/rerun-io/rerun/pull/4518)
  - `DataLoader`s 3: add 3D point cloud `DataLoader` (`.ply`) [#4519](https://github.com/rerun-io/rerun/pull/4519)
  - `DataLoader`s 4: add generic folder `DataLoader` [#4520](https://github.com/rerun-io/rerun/pull/4520)
  - `DataLoader`s 5: add support for external binary `DataLoader`s (PATH) [#4521](https://github.com/rerun-io/rerun/pull/4521)
  - `DataLoader`s 6: first-class support for `Incompatible` [#4565](https://github.com/rerun-io/rerun/pull/4565)
  - `DataLoader`s 7: support for custom `DataLoader`s [#4566](https://github.com/rerun-io/rerun/pull/4566)
- 3D->2D & 2D->3D selection visualizations stick now around on selection [#4587](https://github.com/rerun-io/rerun/pull/4587)
- The viewer now supports segmentation images logged natively as floats [#4585](https://github.com/rerun-io/rerun/pull/4585)
- Fix incorrect bounding box calculation for camera view parts [#4640](https://github.com/rerun-io/rerun/pull/4640)

#### 🚀 Performance Improvements
- Parallelize Space View system evaluation [#4460](https://github.com/rerun-io/rerun/pull/4460)
- Limit server memory [#4636](https://github.com/rerun-io/rerun/pull/4636)

#### 🧑‍🏫 Examples
- Add nuScenes-based lidar examples [#4407](https://github.com/rerun-io/rerun/pull/4407) (thanks [@roym899](https://github.com/roym899)!)
- Nightly builds [#4505](https://github.com/rerun-io/rerun/pull/4505)
- Add LLM token classification example [#4541](https://github.com/rerun-io/rerun/pull/4541) (thanks [@roym899](https://github.com/roym899)!)

#### 📚 Docs
- Shared recordings 3: add how-to guide [#4385](https://github.com/rerun-io/rerun/pull/4385)
- Document our crate organization in ARCHITECTURE.md [#4458](https://github.com/rerun-io/rerun/pull/4458)

#### 🖼 UI Improvements
- Plot legend visibility and position control (part 1): route `EntityProperties` to `SpaceViewClass` methods [#4363](https://github.com/rerun-io/rerun/pull/4363)
- Plot legend visibility and position control (part 2): minor UI spacing improvement [#4364](https://github.com/rerun-io/rerun/pull/4364)
- Reset accumulated bounding box when resetting camera [#4369](https://github.com/rerun-io/rerun/pull/4369)
- Plot legend visibility and position control (part 3): legend UI added for both timeseries and bar charts space views [#4365](https://github.com/rerun-io/rerun/pull/4365)
- Improve component data table UI in the selection panel [#4370](https://github.com/rerun-io/rerun/pull/4370)
- Add optional color component to BarChart archetype [#4372](https://github.com/rerun-io/rerun/pull/4372)
- Resolve unexpected view-partitioning by only bucket images when creating a new 2d view [#4361](https://github.com/rerun-io/rerun/pull/4361)
- Restore `egui_plot` auto-bounds state after dragging the time cursor in timeseries space views [#4270](https://github.com/rerun-io/rerun/pull/4270)
- Make Space View containers selectable and editable [#4403](https://github.com/rerun-io/rerun/pull/4403)
- Improve selection and hover behavior of viewport's tabs [#4424](https://github.com/rerun-io/rerun/pull/4424)
- Improve the Selection Panel UI for components when a single item is selected [#4416](https://github.com/rerun-io/rerun/pull/4416)
- Show connection status in top bar [#4443](https://github.com/rerun-io/rerun/pull/4443)
- Add the possibility to add empty space views of all registered types [#4467](https://github.com/rerun-io/rerun/pull/4467)
- Add experimental Dataframe Space View [#4468](https://github.com/rerun-io/rerun/pull/4468)
- Show e2e latency in metric ui in top panel [#4502](https://github.com/rerun-io/rerun/pull/4502)
- Show leading slash when formatting entity paths [#4537](https://github.com/rerun-io/rerun/pull/4537)
- Improve entity size stats: include whole subtree [#4542](https://github.com/rerun-io/rerun/pull/4542)
- Add support for modal windows to `re_ui` and use it for the Space View entity picker [#4577](https://github.com/rerun-io/rerun/pull/4577)
- Show entity path parts (entity "folder" names) unescaped in UI [#4603](https://github.com/rerun-io/rerun/pull/4603)
- Improve Rerun Menu with link to Rerun Discord [#4661](https://github.com/rerun-io/rerun/pull/4661)
- Introduce container icons and update space views and UI icons [#4663](https://github.com/rerun-io/rerun/pull/4663)
- Initial support for manually adding container and space view in the hierarchy [#4616](https://github.com/rerun-io/rerun/pull/4616)
- Change modal position to a fixed vertical distance from the top of the window [#4700](https://github.com/rerun-io/rerun/pull/4700)

#### 🕸️ Web
- Load examples manifest via HTTP [#4391](https://github.com/rerun-io/rerun/pull/4391)
- Remove builds and usage of `demo.rerun.io` [#4418](https://github.com/rerun-io/rerun/pull/4418)
- Open all links in a new tab [#4582](https://github.com/rerun-io/rerun/pull/4582)

#### 🎨 Renderer Improvements
- Log wgpu adapter on web [#4414](https://github.com/rerun-io/rerun/pull/4414)
- Interior mutability for re_renderer's static resource pools (RenderPipeline/Shader/Layouts/etc.) [#4421](https://github.com/rerun-io/rerun/pull/4421)
- Make draw data creation no longer require a mutable re_renderer context [#4422](https://github.com/rerun-io/rerun/pull/4422)
- Move re_renderer examples to its own crate in order to make workspace level examples less confusing [#4472](https://github.com/rerun-io/rerun/pull/4472)
- Improved wgpu error handling, no more crashes through wgpu validation errors [#4509](https://github.com/rerun-io/rerun/pull/4509)
- Expose `wgpu` profiling scopes to puffin [#4581](https://github.com/rerun-io/rerun/pull/4581)
- Improve shading with two lights instead of one [#4648](https://github.com/rerun-io/rerun/pull/4648)

#### 🧑‍💻 Dev-experience
- Fix not tracking wgsl file changes for web build [#4374](https://github.com/rerun-io/rerun/pull/4374)
- Auto format all the things [#4373](https://github.com/rerun-io/rerun/pull/4373)
- Refactor naming of `SpaceViewClass` and changed `TextSpaceView` name to "Text Log" [#4386](https://github.com/rerun-io/rerun/pull/4386)
- Local-first wheel publishing [#4454](https://github.com/rerun-io/rerun/pull/4454)
- Remove backtraces on error when running `rerun` binary [#4746](https://github.com/rerun-io/rerun/pull/4746)

#### 🗣 Refactors
- Selection state is now fully double buffered and has interior mutability [#4387](https://github.com/rerun-io/rerun/pull/4387)
- Time control is now behind a RwLock, making recording config access non-mutable everywhere [#4389](https://github.com/rerun-io/rerun/pull/4389)
- Enable (selected) new cargo clippy lints [#4404](https://github.com/rerun-io/rerun/pull/4404)
- Add lint for builder pattern functions and deref impls to be marked `#[inline]` [#4435](https://github.com/rerun-io/rerun/pull/4435)
- Pass viewer context always non-mutable [#4438](https://github.com/rerun-io/rerun/pull/4438)
- RenderContext usage cleanup [#4446](https://github.com/rerun-io/rerun/pull/4446)
- Integrate re_tensor_ops crate into re_space_view_tensor [#4450](https://github.com/rerun-io/rerun/pull/4450)
- Use TOML for example readme front-matter [#4553](https://github.com/rerun-io/rerun/pull/4553)
- Rename `StoreDb` to `EntityDb`, `re_data_store` -> `re_entity_db` [#4670](https://github.com/rerun-io/rerun/pull/4670)
- Rename `re_arrow_store` to `re_data_store` [#4672](https://github.com/rerun-io/rerun/pull/4672)

#### 📦 Dependencies
- Update egui and wgpu [#4111](https://github.com/rerun-io/rerun/pull/4111)
- Update Rust to 1.74.0 [#4390](https://github.com/rerun-io/rerun/pull/4390)

#### 🤷‍♂️ Other
- Use `:` instead of `.` as the entity:component separator in paths [#4471](https://github.com/rerun-io/rerun/pull/4471)
- File-like entity paths [#4476](https://github.com/rerun-io/rerun/pull/4476)
- Make the new container blueprints the default behavior [#4642](https://github.com/rerun-io/rerun/pull/4642)


## [0.11.0](https://github.com/rerun-io/rerun/compare/0.10.1...0.11.0) - C++ improvements & better Visible History - 2023-11-28

### Overview & Highlights

- 🌊 C++ SDK improvements
  - [Reference docs are live!](https://ref.rerun.io/docs/cpp/)
  - 2x-5x faster logging
  - CMake install support and other CMake setup improvements
  - Support for custom components & archetypes
  - Zero copy logging for images, various API improvements
- 📈 Visual History -> Visual Time Range
  - Time series plots can now limit its query to a range
  - Much more powerful UI, allowing query ranges relative to time cursor
- 🕸️ The viewer can now be easily embedded in your web apps via our [npm package](https://www.npmjs.com/package/@rerun-io/web-viewer)
- 🐍 ⚠️ Legacy Python API now removed, check the [migration guide](https://github.com/rerun-io/rerun/issues/723) if you're not using `rr.log` yet
- 🦀 The new `StoreSubscriber` trait allows to be notified of all changes in the datastore. This can be used to build custom indices and trigger systems, and serves as a foundation for upcoming performance improvements. Check out [our example](https://github.com/rerun-io/rerun/blob/main/examples/rust/custom_store_subscriber/README.md) for more information.

⚠️ Known issues on Visual Time Range:
- Time cursor [sometimes stops scrolling correctly](https://github.com/rerun-io/rerun/issues/4246) on plot window
- Still [doesn't work with transforms](https://github.com/rerun-io/rerun/issues/723)

Special thanks to @dvad & @dangush for contributing!

### Details

#### 🌊 C++ SDK
- Support std::chrono types for `set_time` on `rerun::RecordingStream` [#4134](https://github.com/rerun-io/rerun/pull/4134)
- Improve rerun_cpp readme & CMakeLists.txt [#4126](https://github.com/rerun-io/rerun/pull/4126)
- Replace the many parameters of  `rerun::spawn` / `rerun::RecordingStream::spawn` with a `struct` [#4149](https://github.com/rerun-io/rerun/pull/4149)
- Make on TextLogLevel PascalCase (instead of SCREAMING CASE) to avoid clashes with preprocessor defines [#4152](https://github.com/rerun-io/rerun/pull/4152)
- Reduce rerun_c library size (by depending on fewer unnecessary crates) [#4147](https://github.com/rerun-io/rerun/pull/4147)
- Fix unnecessary includes in code generated headers [#4132](https://github.com/rerun-io/rerun/pull/4132)
- Doxygen documentation & many doc improvements [#4191](https://github.com/rerun-io/rerun/pull/4191)
- Rename `rerun::ComponentBatch` to `rerun::Collection` (and related constructs) [#4236](https://github.com/rerun-io/rerun/pull/4236)
- Use `rerun::Collection` almost everywhere we'd use `std::vector` before [#4247](https://github.com/rerun-io/rerun/pull/4247)
- Significantly improve C++ logging performance by using C FFI instead of arrow IPC [#4273](https://github.com/rerun-io/rerun/pull/4273)
- Further improve C++ logging for many individual log calls by introducing a component type registry [#4296](https://github.com/rerun-io/rerun/pull/4296)
- All C++ datatypes & components now implement a new Loggable trait [#4305](https://github.com/rerun-io/rerun/pull/4305)
- Add C++ Custom Component example [#4309](https://github.com/rerun-io/rerun/pull/4309)
- Expose Rerun source/include dir in CMakeLists.txt (`RERUN_CPP_SOURCE_DIR`) [#4313](https://github.com/rerun-io/rerun/pull/4313)
- Support cmake install [#4326](https://github.com/rerun-io/rerun/pull/4326)
- Export TensorBuffer & TensorDimension to rerun namespace [#4331](https://github.com/rerun-io/rerun/pull/4331)
- C++ SDK sanity checks now header/source version against rerun_c binary version [#4330](https://github.com/rerun-io/rerun/pull/4330)
- Allow creating Image/Tensor/DepthImage/SegmentationImage directly from shape & pointer [#4345](https://github.com/rerun-io/rerun/pull/4345)

#### 🐍 Python SDK
- Python: remove legacy APIs [#4037](https://github.com/rerun-io/rerun/pull/4037)
- Remove deprecated `rerun_demo` package [#4293](https://github.com/rerun-io/rerun/pull/4293)
- Python: don't catch `KeyboardInterrupt` and `SystemExit` [#4333](https://github.com/rerun-io/rerun/pull/4333) (thanks [@Dvad](https://github.com/Dvad)!)

#### 🪳 Bug Fixes
- Fix line & points (& depth clouds points) radii being unaffected by scale & projection via Pinhole [#4199](https://github.com/rerun-io/rerun/pull/4199)
- Fix inaccessible entities being incorrectly added to space view [#4226](https://github.com/rerun-io/rerun/pull/4226)
- Silence spammy blueprint warnings and validate blueprint on load [#4303](https://github.com/rerun-io/rerun/pull/4303)
- Fix markdown heading size [#4178](https://github.com/rerun-io/rerun/pull/4178)

#### 🌁 Viewer Improvements
- Add command to copy direct link to fully qualified URL [#4165](https://github.com/rerun-io/rerun/pull/4165)
- Implement recording/last-modified-at aware garbage collection [#4183](https://github.com/rerun-io/rerun/pull/4183)

#### 🖼 UI Improvements
- Improve Visible History to support more general time queries [#4123](https://github.com/rerun-io/rerun/pull/4123)
- Add support for Visible History to time series space views [#4179](https://github.com/rerun-io/rerun/pull/4179)
- Make Visible History UI more ergonomic and show inherited values [#4222](https://github.com/rerun-io/rerun/pull/4222)
- Display Visible History on timeline when the mouse hovers the UI [#4259](https://github.com/rerun-io/rerun/pull/4259)
- Improve the Selection Panel with better title, context, and Space View key properties [#4324](https://github.com/rerun-io/rerun/pull/4324)

#### 🕸️ Web
- Put web viewer on `npm` [#4003](https://github.com/rerun-io/rerun/pull/4003)
- Auto-switch port when getting AddrInUse error [#4314](https://github.com/rerun-io/rerun/pull/4314) (thanks [@dangush](https://github.com/dangush)!)
- Generate per-PR web apps [#4341](https://github.com/rerun-io/rerun/pull/4341)

#### 🧑‍💻 Dev-experience
- Simple logging benchmarks for C++ & Rust [#4181](https://github.com/rerun-io/rerun/pull/4181)
- New debug option to show the blueprint in the streams view [#4189](https://github.com/rerun-io/rerun/pull/4189)
- Use pixi over setup scripts on CI + local dev [#4302](https://github.com/rerun-io/rerun/pull/4302)
- Run deploy docs jobs serially [#4232](https://github.com/rerun-io/rerun/pull/4232)
- fix windows test config on main [#4242](https://github.com/rerun-io/rerun/pull/4242)

#### 🗣 Refactors
- `StoreView` -> `StoreSubscriber` [#4234](https://github.com/rerun-io/rerun/pull/4234)
- `DataStore` introduce `StoreEvent`s [#4203](https://github.com/rerun-io/rerun/pull/4203)
- `DataStore` introduce `StoreView`s [#4205](https://github.com/rerun-io/rerun/pull/4205)


## [0.10.1](https://github.com/rerun-io/rerun/compare/0.10.0...0.10.1) - 2023-11-02

### Overview & Highlights
This is a small release primarily to tie up some loose ends for our C++ SDK.

#### 🌊 C++ SDK
- Avoid possible link/symbol errors but defaulting all OSes to static linking of arrow [#4101](https://github.com/rerun-io/rerun/pull/4101)
- Fix compilation errors with C++20 [#4098](https://github.com/rerun-io/rerun/pull/4098)
- Improve C++ SDK perf 5x by respecting CMAKE_BUILD_TYPE and enabling mimalloc [#4094](https://github.com/rerun-io/rerun/pull/4094)
- Reduce amount of cmake log from building & downloading libArrow [#4103](https://github.com/rerun-io/rerun/pull/4103)

#### 🧑‍💻 Dev-experience
- C++ Windows CI [#4110](https://github.com/rerun-io/rerun/pull/4110)
- Add MacOS C++ CI, add Linux C++20 CI [#4120](https://github.com/rerun-io/rerun/pull/4120)


## [0.10.0](https://github.com/rerun-io/rerun/compare/0.9.1...0.10.0) - C++ SDK - 2023-10-30

[Rerun](https://www.rerun.io/) is an easy-to-use visualization toolbox for computer vision and robotics.

* Python: `pip install rerun-sdk`
* Rust: `cargo add rerun` and `cargo install rerun-cli`
* Online demo: <https://app.rerun.io/version/0.10.0/>

Release blog post: <https://www.rerun.io/blog/cpp-sdk>

### Overview & Highlights
* The C++ SDK is finally here!
  ```cpp
  #include <rerun.hpp>

  int main() {
      const auto rec = rerun::RecordingStream("rerun_example_points3d_simple");
      rec.spawn().exit_on_failure();

      rec.log("points", rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
  }
  ```

* Add an integrated getting-started guide into the viewer splash screen
* Add a new and improved `spawn` method in the Rust SDK
* Add support for NV12-encoded images [#3541](https://github.com/rerun-io/rerun/pull/3541) (thanks [@zrezke](https://github.com/zrezke)!)
* We now publish pre-built binaries for each release at <https://github.com/rerun-io/rerun/releases>

### Details
#### 🌊 C++ SDK
- Has all the features of the Python and C++ SDK:s

#### 🐍 Python SDK
- Add `RERUN_STRICT` environment variable [#3861](https://github.com/rerun-io/rerun/pull/3861)
- Fix potential deadlock when saving to file after logging at the end of a Python program [#3920](https://github.com/rerun-io/rerun/pull/3920)
- Warn if no resolution provided to Pinhole [#3923](https://github.com/rerun-io/rerun/pull/3923)
- Python: remove unconditional sleep on `spawn` [#4010](https://github.com/rerun-io/rerun/pull/4010)
- Support `pathlib.Path` for `rr.save` [#4036](https://github.com/rerun-io/rerun/pull/4036)
- Add `disable_timeline` function [#4068](https://github.com/rerun-io/rerun/pull/4068)
- Support fast install of the rerun viewer with `cargo binstall rerun-cli` thanks to [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)

#### 🦀 Rust SDK
- Introduce `re_types_core` [#3878](https://github.com/rerun-io/rerun/pull/3878)
- Fix crash when using `RecordingStream::set_thread_local` on macOS [#3929](https://github.com/rerun-io/rerun/pull/3929)
- Add improved `spawn` function [#3996](https://github.com/rerun-io/rerun/pull/3996) [#4031](https://github.com/rerun-io/rerun/pull/4031)
- Redesign `clap` integration [#3997](https://github.com/rerun-io/rerun/pull/3997) [#4040](https://github.com/rerun-io/rerun/pull/4040)
- `RecordingStream`: introduce `connect_opts` [#4042](https://github.com/rerun-io/rerun/pull/4042)
- Add `disable_timeline` function [#4068](https://github.com/rerun-io/rerun/pull/4068)

#### 🪳 Bug Fixes
- Fix grayscale images being too dark [#3999](https://github.com/rerun-io/rerun/pull/3999)
- Prevent badly sized tensors from crashing the viewer [#4005](https://github.com/rerun-io/rerun/pull/4005)
- Fix selection history right-click menu not working [#3819](https://github.com/rerun-io/rerun/pull/3819)

#### 🌁 Viewer Improvements
- Replace `--strict` flag with `RERUN_PANIC_ON_WARN` env-var [#3872](https://github.com/rerun-io/rerun/pull/3872)
- Support NV12-encoded images [#3541](https://github.com/rerun-io/rerun/pull/3541) (thanks [@zrezke](https://github.com/zrezke)!)

#### 🧑‍🏫 Examples
- `--max-frame` support for tracking examples [#3835](https://github.com/rerun-io/rerun/pull/3835)

#### 📚 Docs
- Synchronize code examples and their screenshots [#3954](https://github.com/rerun-io/rerun/pull/3954)
- Improve docs for `TextDocument` example [#4008](https://github.com/rerun-io/rerun/pull/4008)
- Fix typos in documentation and code comments [#4061](https://github.com/rerun-io/rerun/pull/4061) (thanks [@omahs](https://github.com/omahs)!)

#### 🖼 UI Improvements
- Add basic support for in-app "Quick Start" guides [#3813](https://github.com/rerun-io/rerun/pull/3813) [#3912](https://github.com/rerun-io/rerun/pull/3912)
- Add copy-button to markdown code blocks [#3882](https://github.com/rerun-io/rerun/pull/3882)
- Add warning in the Quick Start guides about Safari breaking Copy to Clipboard [#3898](https://github.com/rerun-io/rerun/pull/3898)

#### 🎨 Renderer Improvements
- Add easy way to dump out final wgsl shader [#3947](https://github.com/rerun-io/rerun/pull/3947)

#### 🧑‍💻 Dev-experience
- Approve all workflow runs for a specific contributor PR [#3876](https://github.com/rerun-io/rerun/pull/3876)
- Make codegen I/O-free and agnostic to output location [#3888](https://github.com/rerun-io/rerun/pull/3888)
- Configure pytest to fail on warnings [#3903](https://github.com/rerun-io/rerun/pull/3903)
- Improve `taplo` output on failure [#3909](https://github.com/rerun-io/rerun/pull/3909)
- Automatically synchronize build.rerun.io & release assets [#3945](https://github.com/rerun-io/rerun/pull/3945)
- New helper script to run fast lints and pre-push hook that runs it [#3949](https://github.com/rerun-io/rerun/pull/3949)
- CI: Rerun CLI as a release asset [#3959](https://github.com/rerun-io/rerun/pull/3959)
- Add script to generate RRD vs. screenshots comparisons [#3946](https://github.com/rerun-io/rerun/pull/3946)
- Add a new build Environment option for CondaBuild to improve conda-built artifacts [#4015](https://github.com/rerun-io/rerun/pull/4015)
- Lock python in CI to 3.11 [#4033](https://github.com/rerun-io/rerun/pull/4033)
- Changed `spawn()` and the `rerun` script to call into `rerun_bindings` (12x startup time improvement) [#4053](https://github.com/rerun-io/rerun/pull/4053)


## [0.9.1](https://github.com/rerun-io/rerun/compare/0.9.0...0.9.1) - Bug fixes and performance improvements - 2023-10-12

[Rerun](https://www.rerun.io/) is an easy-to-use visualization toolbox for computer vision and robotics.

* Python: `pip install rerun-sdk`
* Rust: `cargo add rerun` and `cargo install rerun-cli`
* Online demo: <https://app.rerun.io/version/0.9.1/>

### Overview & Highlights
- A bunch of bug fixes
- Fix big performance regression when hovering images
- The Rerun Viewer should now be visible to the system accessibility system

#### 🐍 Python SDK
- Added support for PyTorch array to `Boxes2D`'s `array` convenience argument [#3719](https://github.com/rerun-io/rerun/pull/3719)
- Fix default stroke width handling in `log_line_strip_Xd` and `log_obbs` [#3720](https://github.com/rerun-io/rerun/pull/3720)
- Warn/raise when passing incompatible objects to `log` [#3727](https://github.com/rerun-io/rerun/pull/3727)
- Refactor `rerun.AnyValues` to handle `None` input more gracefully [#3725](https://github.com/rerun-io/rerun/pull/3725)
- Default `DisconnectedSpaces` boolean to `true` in Python [#3760](https://github.com/rerun-io/rerun/pull/3760)

#### 🦀 Rust SDK
- Fix return type of `entity_path!()` and `entity_path_vec!()` on empty input [#3734](https://github.com/rerun-io/rerun/pull/3734) (thanks [@kpreid](https://github.com/kpreid)!)
- Export `RecordingStreamError` [#3777](https://github.com/rerun-io/rerun/pull/3777)

#### 🪳 Bug Fixes
- Fix bug when joining cleared optional components [#3726](https://github.com/rerun-io/rerun/pull/3726)
- Update `winit` to 0.28.7 to fix UI glitch on macOS Sonoma [#3763](https://github.com/rerun-io/rerun/pull/3763)
- Show 1D-tensors as bar charts [#3769](https://github.com/rerun-io/rerun/pull/3769)
- Fix loading of `.obj` mesh files [#3772](https://github.com/rerun-io/rerun/pull/3772)
- Fix crash when loading huge image [#3775](https://github.com/rerun-io/rerun/pull/3775)
- Fix performance regression when viewing images and tensors [#3767](https://github.com/rerun-io/rerun/pull/3767)

#### 🌁 Viewer Improvements
- Turn on `AccessKit` accessibility integration [#3732](https://github.com/rerun-io/rerun/pull/3732)
- Display space views using `ViewCoordinates` from closest ancestor [#3748](https://github.com/rerun-io/rerun/pull/3748)
- Improve 3D view bounds handling of camera frustums [#3749](https://github.com/rerun-io/rerun/pull/3749) [#3815](https://github.com/rerun-io/rerun/pull/3815) [#3811](https://github.com/rerun-io/rerun/pull/3811)
- Improve heuristics around 2D vs 3D space-view creation [#3822](https://github.com/rerun-io/rerun/pull/3822)

#### 🚀 Performance Improvements
- Optimize gathering of point cloud colors [#3730](https://github.com/rerun-io/rerun/pull/3730)

#### 🧑‍🏫 Examples
- Fix open photogrammetry example not working on Windows [#3705](https://github.com/rerun-io/rerun/pull/3705)

#### 📚 Docs
- Document that entity-path `rerun/` is reserved [#3747](https://github.com/rerun-io/rerun/pull/3747)

#### 🖼 UI Improvements
- Show all entities/components in the Streams UI, even if empty for the selected timeline [#3779](https://github.com/rerun-io/rerun/pull/3779)

#### 🧑‍💻 Dev-experience
- Less automatic `build.rs` shenanigans [#3814](https://github.com/rerun-io/rerun/pull/3814)

#### 🗣 Refactors
- Refactor our `build.rs` files [#3789](https://github.com/rerun-io/rerun/pull/3789)

#### 📦 Dependencies
- Update `ewebsock` to 0.4.0 [#3729](https://github.com/rerun-io/rerun/pull/3729)
- Update `winit` to 0.28.7 [#3763](https://github.com/rerun-io/rerun/pull/3763)


## [0.9.0](https://github.com/rerun-io/rerun/compare/0.8.2...0.9.0) - New logging API - 2023-10-05

[Rerun](https://www.rerun.io/) is an easy-to-use visualization toolbox for computer vision and robotics.

* Python: `pip install rerun-sdk`
* Rust: `cargo add rerun` and `cargo install rerun-cli`
* Online demo: <https://app.rerun.io/version/0.9.0/>


### Overview & Highlights
Rerun 0.9.0 is a big release, that introduces a brand new logging API.
This API is code-generated from a common definition, meaning the Python and Rust SDKs are very similar now.
This will let us more easily extend and improve the API going forward.
It is also the basis for our C++ API, which is coming in Rerun 0.10.0.

Read [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for details!

<picture>
  <img src="https://static.rerun.io/0.9.0-start-screen/ee485acc4bf50519102180d01ae6338aef07e88e/full.png" alt="0.9.0 Welcome Screen">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/0.9.0-start-screen/ee485acc4bf50519102180d01ae6338aef07e88e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/0.9.0-start-screen/ee485acc4bf50519102180d01ae6338aef07e88e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/0.9.0-start-screen/ee485acc4bf50519102180d01ae6338aef07e88e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/0.9.0-start-screen/ee485acc4bf50519102180d01ae6338aef07e88e/1200w.png">
</picture>

Other highlights:
* 🏃‍♀️ Large point clouds are up to 3x faster now
* 📚 Markdown view support
  * 🔗 with easy to use in-viewer entity & component links
* 📺 New startup screen
* 🐛 Lots and lots of bugfixes
  * 👷‍♀️ Internally we have now way more automated testing for the new API surfaces
* ✨ drag & drop for images & meshes (even on web!), time display in local time (thanks @jparismorgan!),
  .obj mesh support, default enabled memory limit, new how-to guide for custom data… and many more smaller features!

### Some select details
#### 🐍 Python SDK
- Handle older numpy versions / py 3.8 in `VecND` extensions [#2896](https://github.com/rerun-io/rerun/pull/2896)
- Add default value for `info` argument of `ClassDescription` [#3017](https://github.com/rerun-io/rerun/pull/3017)
- Run all Python doc examples in CI [#3172](https://github.com/rerun-io/rerun/pull/3172)
- Create objects for delegating components [#3303](https://github.com/rerun-io/rerun/pull/3303)
- Allow any string as an entity path [#3443](https://github.com/rerun-io/rerun/pull/3443)
- Check if another process is already listening on the port before trying to spawn [#3501](https://github.com/rerun-io/rerun/pull/3501)
- Force kw-args on more Python functions [#3515](https://github.com/rerun-io/rerun/pull/3515)
- Deprecate all of the legacy `log_` prefixed APIs. [#3564](https://github.com/rerun-io/rerun/pull/3564)
- Introduce AnyValues as an alternative to extension_components [#3561](https://github.com/rerun-io/rerun/pull/3561)

#### 🦀 Rust SDK
- Introduce versioned `EntityPath` & refactor mesh/tensor caching [#3230](https://github.com/rerun-io/rerun/pull/3230)
- Make `FileSink` actually flush its data when asked to [#3525](https://github.com/rerun-io/rerun/pull/3525)
- `TextLog` integrations with native loggers [#3522](https://github.com/rerun-io/rerun/pull/3522)

#### 🪳 Bug Fixes
- Fix bug in size estimation of array buffers [#2991](https://github.com/rerun-io/rerun/pull/2991)
- Fix the Streams UI when the recording is empty [#3027](https://github.com/rerun-io/rerun/pull/3027)
- Clamp time panel height to avoid visual glitches [#3169](https://github.com/rerun-io/rerun/pull/3169)
- Allow user to edit colormap for depth images [#3241](https://github.com/rerun-io/rerun/pull/3241)
- Fix lurking bug in datastore bucket sorting routines [#3281](https://github.com/rerun-io/rerun/pull/3281)
- Fix row ordering flakiness when using clear APIs [#3288](https://github.com/rerun-io/rerun/pull/3288)
- Fix incorrect propagation of field's nullability into its inner list [#3352](https://github.com/rerun-io/rerun/pull/3352)
- Fix post-GC purging of streams view time histogram [#3364](https://github.com/rerun-io/rerun/pull/3364)
- Fix color grayscale colormap not being even [#3391](https://github.com/rerun-io/rerun/pull/3391)
- Fix depth point cloud not taking transformation at its path into account [#3514](https://github.com/rerun-io/rerun/pull/3514)
- Fix infinite recursion when putting a container inside a viewer tab [#3534](https://github.com/rerun-io/rerun/pull/3534)
- Fix failing to preview small images [#3520](https://github.com/rerun-io/rerun/pull/3520)

#### 🌁 Viewer Improvements
- Open image and mesh files with drag-drop and File->Open [#3116](https://github.com/rerun-io/rerun/pull/3116)
- Support loading images and meshes on web [#3131](https://github.com/rerun-io/rerun/pull/3131)
- Add `rerun reset` command [#3145](https://github.com/rerun-io/rerun/pull/3145)
- Show picking position when hovering something in the spatial view [#3227](https://github.com/rerun-io/rerun/pull/3227)
- Rethink view selection & filtering + make all views opt-in [#3323](https://github.com/rerun-io/rerun/pull/3323)
- Markdown support in `TextDocument` [#3343](https://github.com/rerun-io/rerun/pull/3343)
- Click `recording://entity/path` links in markdown [#3442](https://github.com/rerun-io/rerun/pull/3442)
- Allow showing image shaped tensors in the tensor view [#3583](https://github.com/rerun-io/rerun/pull/3583)
- Add option to display timestamps in the local system timezone [#3530](https://github.com/rerun-io/rerun/pull/3530) (thanks [@jparismorgan](https://github.com/jparismorgan)!)
- Add obj mesh support to viewer [#3670](https://github.com/rerun-io/rerun/pull/3670)

#### 🚀 Performance Improvements
- Pass through strings using arrow2::Buffers [#2931](https://github.com/rerun-io/rerun/pull/2931)
- Introduce codegen optimizations for primitives and fixed-sized-arrays [#2970](https://github.com/rerun-io/rerun/pull/2970)
- Optimize big point clouds by ~20% [#3108](https://github.com/rerun-io/rerun/pull/3108)
- A nice speed up of 3D points clouds by ~69% [#3114](https://github.com/rerun-io/rerun/pull/3114)
- Improve performance for many entities [#3078](https://github.com/rerun-io/rerun/pull/3078)
- Turn on garbage-collection (`--memory-limit`) by default [#3161](https://github.com/rerun-io/rerun/pull/3161)
- Optimize out unnecessary joins when querying archetypes [#3377](https://github.com/rerun-io/rerun/pull/3377)

#### 🧑‍🏫 Examples
- Add "rerun_example_" prefix to all our user-visible app-ids [#3112](https://github.com/rerun-io/rerun/pull/3112)
- Add paper visualizations to examples [#3020](https://github.com/rerun-io/rerun/pull/3020) (thanks [@roym899](https://github.com/roym899)!)
- API examples overhaul & roundtrip tests [#3204](https://github.com/rerun-io/rerun/pull/3204)
- Generate manifest for examples page in viewer [#3332](https://github.com/rerun-io/rerun/pull/3332)
- Fix `transform3d_simple` and reenable roundtrip test [#3401](https://github.com/rerun-io/rerun/pull/3401)
- Update import path for HuggingFace's `randn_tensor` [#3506](https://github.com/rerun-io/rerun/pull/3506) (thanks [@hu-po](https://github.com/hu-po)!)
- Add ControlNet example [#3568](https://github.com/rerun-io/rerun/pull/3568) (thanks [@roym899](https://github.com/roym899)!)

#### 📚 Docs
- Fix outdated links in docs [#2854](https://github.com/rerun-io/rerun/pull/2854)
- Add how-to guide for clearing entities [#3211](https://github.com/rerun-io/rerun/pull/3211)
- Support `\example` in codegen [#3378](https://github.com/rerun-io/rerun/pull/3378)
- Docs codegen [#3445](https://github.com/rerun-io/rerun/pull/3445)
- Generate component/datatype docs [#3535](https://github.com/rerun-io/rerun/pull/3535)
- Update the python API docs site for the new APIs [#3565](https://github.com/rerun-io/rerun/pull/3565)
- Add a how-to guide for using Rerun with custom data [#3634](https://github.com/rerun-io/rerun/pull/3634)

#### 🖼 UI Improvements
- Migrate to custom checkbox/radio_value UI [#2851](https://github.com/rerun-io/rerun/pull/2851)
- Remove expansion effect from time panel toolbar [#2863](https://github.com/rerun-io/rerun/pull/2863)
- Remove expansion effect from the large collapsing headers [#2864](https://github.com/rerun-io/rerun/pull/2864)
- Change the styling and behavior of hyperlinks [#2872](https://github.com/rerun-io/rerun/pull/2872)
- Improve space view tab design [#2879](https://github.com/rerun-io/rerun/pull/2879)
- Improve drag tab UI [#2893](https://github.com/rerun-io/rerun/pull/2893)
- Normalize various text string in UI [#2902](https://github.com/rerun-io/rerun/pull/2902)
- Add (debug-only) style panel [#2914](https://github.com/rerun-io/rerun/pull/2914)
- Add clip rect in panels and use them for large collapsing headers [#2936](https://github.com/rerun-io/rerun/pull/2936)
- Add Recordings section to the left panel [#2938](https://github.com/rerun-io/rerun/pull/2938)
- New triangle collapse arrow for large collapsible header [#2920](https://github.com/rerun-io/rerun/pull/2920)
- Add support for tree to `ListItem` [#2968](https://github.com/rerun-io/rerun/pull/2968)
- Add hierarchical display in recordings panel [#2971](https://github.com/rerun-io/rerun/pull/2971)
- Add support to close a recording [#2972](https://github.com/rerun-io/rerun/pull/2972)
- Show RAM use and data rate when hovering an entity in stream view [#2997](https://github.com/rerun-io/rerun/pull/2997)
- Don't select the spaceview when maximizing it [#2988](https://github.com/rerun-io/rerun/pull/2988)
- Add delete buttons in the Recordings UI [#2976](https://github.com/rerun-io/rerun/pull/2976)
- Introduce a welcome screen when no recording is loaded [#2982](https://github.com/rerun-io/rerun/pull/2982)
- Remove the limitation to a single dropped file [#3030](https://github.com/rerun-io/rerun/pull/3030)
- Uniform icon, pointer, and tooltip for external links [#3026](https://github.com/rerun-io/rerun/pull/3026)
- Improve styling of demo header [#3022](https://github.com/rerun-io/rerun/pull/3022)
- Implement "Open file" dialog on Web [#3068](https://github.com/rerun-io/rerun/pull/3068)
- Show Welcome Screen after closing recording even with `--skip-welcome-screen` [#3035](https://github.com/rerun-io/rerun/pull/3035)
- Fix the 3d space view's tooltip help text [#3132](https://github.com/rerun-io/rerun/pull/3132)
- Use `ListItem` in blueprint tree UI [#3118](https://github.com/rerun-io/rerun/pull/3118)
- Use `ListItem` in Stream Tree UI [#3153](https://github.com/rerun-io/rerun/pull/3153)
- Limit the size of component tooltips with `UiVerbosity::Reduced` [#3171](https://github.com/rerun-io/rerun/pull/3171)
- Smaller AnnotationContext tooltip [#3217](https://github.com/rerun-io/rerun/pull/3217)
- Add Examples page to the Welcome Screen [#3191](https://github.com/rerun-io/rerun/pull/3191)
- `Welcome Page` refresh [#3219](https://github.com/rerun-io/rerun/pull/3219)
- Show currently loading recordings in Recordings menu [#3307](https://github.com/rerun-io/rerun/pull/3307)
- Update to latest egui + use new Image api [#3311](https://github.com/rerun-io/rerun/pull/3311)
- Hide stream view and selection view in welcome app [#3333](https://github.com/rerun-io/rerun/pull/3333)
- Tighter UI for Pinhole and when hovering images [#3579](https://github.com/rerun-io/rerun/pull/3579)
- Improve viewport tile behavior [#3295](https://github.com/rerun-io/rerun/pull/3295)
- Show color map preview for depth point clouds as well [#3373](https://github.com/rerun-io/rerun/pull/3373)

#### 🕸️ Web
- Move example description to README frontmatter [#3201](https://github.com/rerun-io/rerun/pull/3201)
- Fix instantiateStreaming usage on web [#3209](https://github.com/rerun-io/rerun/pull/3209)
- Web-Viewer: Don't auto-connect to `wss://hostname` when an `?url=` is missing [#3345](https://github.com/rerun-io/rerun/pull/3345)

#### 📈 Analytics
- Recreate the analytics state directory if necessary before creating pipeline [#2878](https://github.com/rerun-io/rerun/pull/2878)
- Update resolved analytics URL [#3101](https://github.com/rerun-io/rerun/pull/3101)
- Use `ehttp` in `re_analytics` [#3155](https://github.com/rerun-io/rerun/pull/3155)
- Web analytics [#3166](https://github.com/rerun-io/rerun/pull/3166)
- Keep track of how files are sourced for analytics and UI [#3371](https://github.com/rerun-io/rerun/pull/3371)

#### 🧑‍💻 Dev-experience
- Make `cargo codegen` work irrelevant of CWD [#2913](https://github.com/rerun-io/rerun/pull/2913)
- `scripts/highlight_issues.py`: print issues with no comments [#2939](https://github.com/rerun-io/rerun/pull/2939)
- Use `prettyplease` to improve formatting of generated Rust code [#2949](https://github.com/rerun-io/rerun/pull/2949)
- Enable debug symbols in build scripts (`build.rs`) in dev mode [#2962](https://github.com/rerun-io/rerun/pull/2962)
- Update egui via a `[patch]` [#2969](https://github.com/rerun-io/rerun/pull/2969)
- Track file sizes [#3037](https://github.com/rerun-io/rerun/pull/3037)
- Fix docs previews [#3066](https://github.com/rerun-io/rerun/pull/3066)
- Name the rayon threads [#3060](https://github.com/rerun-io/rerun/pull/3060)
- Improve size tracking table [#3117](https://github.com/rerun-io/rerun/pull/3117)
- Remove `setup-rust` from toml lint job [#3143](https://github.com/rerun-io/rerun/pull/3143)
- Render demo manifest [#3151](https://github.com/rerun-io/rerun/pull/3151)
- Fix update PR body script [#3181](https://github.com/rerun-io/rerun/pull/3181)
- Update CI `actions/checkout@v4` [#3208](https://github.com/rerun-io/rerun/pull/3208)
- Update all uses of `actions/checkout` to use explicit `ref` [#3322](https://github.com/rerun-io/rerun/pull/3322)
- Make 'Print datastore' viable with real world data [#3452](https://github.com/rerun-io/rerun/pull/3452)
- Update workflows to support fork PRs [#3544](https://github.com/rerun-io/rerun/pull/3544)

#### 🗣 Refactors
- Remove legacy `re_components` [#3440](https://github.com/rerun-io/rerun/pull/3440)

#### 📦 Dependencies
- Update clang-format [#2942](https://github.com/rerun-io/rerun/pull/2942)
- Rust 1.72 + format `let-else` (!) [#3102](https://github.com/rerun-io/rerun/pull/3102)
- Update to egui 0.23 [#3523](https://github.com/rerun-io/rerun/pull/3523)
- Update to wgpu 0.17 [#2980](https://github.com/rerun-io/rerun/pull/2980)

#### 🤷‍♂️ Other
- Always protect at least one value on the timeline when running GC [#3357](https://github.com/rerun-io/rerun/pull/3357)


## [0.8.2](https://github.com/rerun-io/rerun/compare/0.8.1...0.8.2) - Bug fixes - 2023-09-05

#### 🪳 Bug Fixes
- Fix quadratic slowdown when ingesting data with uniform time [#3088](https://github.com/rerun-io/rerun/pull/3088)
- Normalize quaternions [#3094](https://github.com/rerun-io/rerun/pull/3094)
- Improve error message in common `re_renderer` crash [#3070](https://github.com/rerun-io/rerun/pull/3070)
- Fix crash when trying to render too many line segments [#3093](https://github.com/rerun-io/rerun/pull/3093)
- Handle serde-field that fails to deserialize [#3130](https://github.com/rerun-io/rerun/pull/3130)
- GC the blueprints before saving while preserving the current state [#3148](https://github.com/rerun-io/rerun/pull/3148)

#### 🧑‍🏫 Examples
- Make `custom_space_view` example more verbose [#3123](https://github.com/rerun-io/rerun/pull/3123)

#### 🖼 UI Improvements
- Change the "slow-down-camera" modifier to Alt on non-Mac [#3051](https://github.com/rerun-io/rerun/pull/3051) (thanks [@h3mosphere](https://github.com/h3mosphere)!)

#### 🎨 Renderer Improvements
- Warn if using software rasterizer (lavapipe or llvmpipe) [#3134](https://github.com/rerun-io/rerun/pull/3134)

#### 📦 Dependencies
- Update webpki: https://rustsec.org/advisories/RUSTSEC-2023-0052 [#3176](https://github.com/rerun-io/rerun/pull/3176)


## [0.8.1](https://github.com/rerun-io/rerun/compare/0.8.0...0.8.1) - Bug fixes - 2023-08-17

#### 🐍 Python SDK
- Add a warning category and stacklevel to rerun warnings.warn calls [#2985](https://github.com/rerun-io/rerun/pull/2985)

#### 🪳 Bug Fixes
- Fix always redrawing in the presence of a 3D space view [#2900](https://github.com/rerun-io/rerun/pull/2900)
- Fix unable to set camera spinning until camera has moved [#2990](https://github.com/rerun-io/rerun/pull/2990)

#### 🌁 Viewer Improvements
- Allow changing plot aspect ratio with scroll + cmd/ctrl + alt [#2742](https://github.com/rerun-io/rerun/pull/2742)
- Automatically select user timeline if no timeline was explicitly selected yet [#2986](https://github.com/rerun-io/rerun/pull/2986)

#### 🧑‍🏫 Examples
- Add `Helix` to `demo.rerun.io` [#2930](https://github.com/rerun-io/rerun/pull/2930)

#### 📈 Analytics
- Make sure `re_analytics` never log higher than at `debug` level [#3014](https://github.com/rerun-io/rerun/pull/3014)


## [0.8.0](https://github.com/rerun-io/rerun/compare/0.7.0...0.8.0) - Infrastructure investments and more transform improvements - 2023-07-27

[Rerun](https://www.rerun.io/) is an easy-to-use visualization toolbox for computer vision and robotics.

* Python: `pip install rerun-sdk`
* Rust: `cargo add rerun` and `cargo install rerun-cli`
* Online demo: <https://demo.rerun.io/version/0.8.0/>


### Overview & Highlights
 - `log_pinhole` is now easier to use in simple cases and supports non-RDF camera coordinates. [#2614](https://github.com/rerun-io/rerun/pull/2614)
   - You only need to set focal length and optional principal point instead of setting the full 3x3 matrix.
   - There is also a new argument: `camera_xyz` for setting the coordinate system. The default is RDF (the old
   default). This affects the visible camera frustum, how rays are projected when hovering a 2D image, and how depth
   clouds are projected.
 - The visualizer can now show coordinate arrows for all affine transforms within the view. [#2577](https://github.com/rerun-io/rerun/pull/2577)
 - Linestrips and oriented bounding boxes can now be logged via batch APIs in python.
   - See: `log_linestrips_2d`, `log_linestrips_3d`, [#2822](https://github.com/rerun-io/rerun/pull/2822) and `log_obbs` [#2823](https://github.com/rerun-io/rerun/pull/2823)
 - Rust users that build their own Viewer applications can now add fully custom Space Views. Find more information [here](https://www.rerun.io/docs/howto/extend/extend-ui#custom-space-views-classes).
 - New optional `flush_timeout` specifies how long Rerun will wait if a TCP stream is disconnected during a flush. [#2821](https://github.com/rerun-io/rerun/pull/2821)
   - In Rust, `RecordingStream::connect` now requires `flush_timeout` specified as an `Option<Duration>`.
     - To keep default behavior, this can be specified using the `rerun::default_flush_time()` helper.
   - In Python `flush_init_sec` is now an optional argument to `rr.connect()`
 - In Rust, the `RecordingStream` now offers a stateful time API, similar to the Python APIs. [#2506](https://github.com/rerun-io/rerun/pull/2506)
   - You can now call `set_time_sequence`, `set_time_seconds`, and `set_time_nanos` directly on the `RecordingStream`,
     which will set the time for all subsequent logs using that stream.
   - This can be used as an alternative to the previous `MsgSender::with_time` APIs.
 - The Rerun SDK now defaults to 8ms long microbatches instead of 50ms. This makes the default behavior more suitable
for use-cases like real-time video feeds. [#2220](https://github.com/rerun-io/rerun/pull/2220)
   - Check out [the microbatching docs](https://www.rerun.io/docs/reference/sdk-micro-batching) for more information
   on fine-tuning the micro-batching behavior.
 - The web viewer now incremental loads `.rrd` files when streaming over HTTP. [#2412](https://github.com/rerun-io/rerun/pull/2412)

![Open Photogrammetry Preview](https://static.rerun.io/9fa26e73a197690e0403cd35f29e31c2941dea36_release_080_photogrammetry_full.png)

### Ongoing Refactors
 - There have been a number of significant internal changes going on during this release with little visible impact.
   This work will land across future releases, but is highlighted here since much of it is visible through the
   changelog.
   - The layout of the Viewer is now controlled by a Blueprint datastore. In the future this will allow for direct API
    control of the layout and configuration of the Viewer. A very early prototype of this functionality is available
    via the `rerun.experimental` module in Python.
   - An entirely new code-generation framework has been brought online for Rust, Python and C++. This will eventually enable
    new object-centric APIs with a more scalable, consistent, and ergonomic experience.
   - Bringup of C++ support is now underway and will eventually become our third officially supported SDK language.

### Known Regressions
- Due to the Blueprint storage migration, blueprint persistence on web is currently broken. Will be resolved in:
 [#2579](https://github.com/rerun-io/rerun/issues/2579)

### In Detail
#### 🐍 Python SDK
- Clean up warnings printed when `rr.init` hasn't been called [#2209](https://github.com/rerun-io/rerun/pull/2209)
- Normalize Python typing syntax to 3.8+ [#2361](https://github.com/rerun-io/rerun/pull/2361)
- Simpler, sturdier stateful time tracking in both SDKs [#2506](https://github.com/rerun-io/rerun/pull/2506)
- Fix not taking np.array for single colors [#2569](https://github.com/rerun-io/rerun/pull/2569)
- Add a basic pyright config [#2610](https://github.com/rerun-io/rerun/pull/2610)
- Improve `log_pinhole` and support non-RDF pinholes [#2614](https://github.com/rerun-io/rerun/pull/2614)
- Expose batch APIs for linestrips [#2822](https://github.com/rerun-io/rerun/pull/2822)
- Expose batch APIs for oriented bounding boxes [#2823](https://github.com/rerun-io/rerun/pull/2823)

#### 🦀 Rust SDK
- Add example for adding custom Space Views [#2328](https://github.com/rerun-io/rerun/pull/2328)
- Simpler, sturdier stateful time tracking in both SDKs [#2506](https://github.com/rerun-io/rerun/pull/2506)
- Automagic flush when `take()`ing a `MemorySinkStorage` [#2632](https://github.com/rerun-io/rerun/pull/2632)
- Logging SDK: Log warnings if user data is dropped [#2630](https://github.com/rerun-io/rerun/pull/2630)
- Add support for `RecordingStream::serve` [#2815](https://github.com/rerun-io/rerun/pull/2815)

#### 🌁 Viewer Improvements
- Better handle scroll-to-zoom in 3D views [#1764](https://github.com/rerun-io/rerun/pull/1764)
- Add command to screenshot the application [#2293](https://github.com/rerun-io/rerun/pull/2293)
- Show layout in blueprint tree view [#2465](https://github.com/rerun-io/rerun/pull/2465)
- Double-click to select entity [#2504](https://github.com/rerun-io/rerun/pull/2504)
- Add Rerun.io link/text in top bar [#2540](https://github.com/rerun-io/rerun/pull/2540)
- New auto-layout of space views [#2558](https://github.com/rerun-io/rerun/pull/2558)
- Add 'Dump datastore' command to palette [#2564](https://github.com/rerun-io/rerun/pull/2564)
- Support any `dtype` for depth images [#2602](https://github.com/rerun-io/rerun/pull/2602)
- Change "Save Selection" command to Cmd+Alt+S [#2631](https://github.com/rerun-io/rerun/pull/2631)
- Consistent transform visualization for all entities with transforms [#2577](https://github.com/rerun-io/rerun/pull/2577)
- Improve `log_pinhole` and support non-RDF pinholes [#2614](https://github.com/rerun-io/rerun/pull/2614)

#### 🚀 Performance Improvements
- Flush the batches every 8ms instead of 50 ms [#2220](https://github.com/rerun-io/rerun/pull/2220)
- Replace `image` crate jpeg decoder with zune-jpeg [#2376](https://github.com/rerun-io/rerun/pull/2376)
- Stream `.rrd` files when loading via http [#2412](https://github.com/rerun-io/rerun/pull/2412)

#### 🪳 Bug Fixes
- Fix deadlock when misusing the Caches [#2318](https://github.com/rerun-io/rerun/pull/2318)
- Fix unstable order/flickering of "shown in" space view list on selection [#2327](https://github.com/rerun-io/rerun/pull/2327)
- Fix transforms not applied to connections from transform context [#2407](https://github.com/rerun-io/rerun/pull/2407)
- Fix texture clamping and color gradient selection being displayed incorrectly [#2394](https://github.com/rerun-io/rerun/pull/2394)
- Fix projected ray length [#2482](https://github.com/rerun-io/rerun/pull/2482)
- Tweak the depth bias multiplier for WebGL [#2491](https://github.com/rerun-io/rerun/pull/2491)
- Clip image zoom rectangle [#2505](https://github.com/rerun-io/rerun/pull/2505)
- Fix missing feature flags for benchmarks [#2515](https://github.com/rerun-io/rerun/pull/2515)
- `run_all.py` script fixes [#2519](https://github.com/rerun-io/rerun/pull/2519)
- Update egui_tiles with fix for drag-and-drop-panic [#2555](https://github.com/rerun-io/rerun/pull/2555)
- Convert objectron proto.py back to using typing.List [#2559](https://github.com/rerun-io/rerun/pull/2559)
- Exclude from `objectron/proto/objectron/proto.py` from `just py-format` [#2562](https://github.com/rerun-io/rerun/pull/2562)
- Fix pinhole visualization not working with camera extrinsics & intrinsics on the same path [#2568](https://github.com/rerun-io/rerun/pull/2568)
- Fix: always auto-layout spaceviews until the user intervenes [#2583](https://github.com/rerun-io/rerun/pull/2583)
- Fix freeze/crash when logging large times [#2588](https://github.com/rerun-io/rerun/pull/2588)
- Update egui_tiles to fix crash [#2598](https://github.com/rerun-io/rerun/pull/2598)
- Fix clicking object with single instance (of every component) selecting instance instead of entity [#2573](https://github.com/rerun-io/rerun/pull/2573)
- Cleanup internal data-structures when process has been forked [#2676](https://github.com/rerun-io/rerun/pull/2676)
- Fix shutdown race-condition by introducing a flush_timeout before dropping data [#2821](https://github.com/rerun-io/rerun/pull/2821)
- Fix ui-scale based point/line sizes incorrectly scaled when zooming based on horizontal dimension [#2805](https://github.com/rerun-io/rerun/pull/2805)
- Fix visibility toggle for maximized Space Views [#2806](https://github.com/rerun-io/rerun/pull/2806)
- Fix loading file via CLI [#2807](https://github.com/rerun-io/rerun/pull/2807)
- Fix disconnected space APIs in Python SDK [#2832](https://github.com/rerun-io/rerun/pull/2832)
- Avoid unwrap when generating authkey [#2804](https://github.com/rerun-io/rerun/pull/2804)

#### 🧑‍🏫 Examples
- Add example template [#2392](https://github.com/rerun-io/rerun/pull/2392)
- Show hidden url search param in `app.rerun.io` [#2455](https://github.com/rerun-io/rerun/pull/2455)
- Minimal example of running an intel realsense depth sensor live [#2541](https://github.com/rerun-io/rerun/pull/2541)
- Add a simple example to display Open Photogrammetry Format datasets [#2512](https://github.com/rerun-io/rerun/pull/2512)
- Move `examples/api_demo` -> `tests/test_api` [#2585](https://github.com/rerun-io/rerun/pull/2585)

#### 📚 Docs
- Docs: link to `rr.save` and suggest `rerun` instead of `python -m rerun` [#2586](https://github.com/rerun-io/rerun/pull/2586)
- Update docs about transforms [#2496](https://github.com/rerun-io/rerun/pull/2496)
- Fixup remaining usages of log_rigid3 in docs [#2831](https://github.com/rerun-io/rerun/pull/2831)

#### 🎨 Renderer Improvements
- Expose type erased draw data that can be consumed directly [#2300](https://github.com/rerun-io/rerun/pull/2300)
- Use less `mut` when using `RenderContext` [#2312](https://github.com/rerun-io/rerun/pull/2312)

#### 🧑‍💻 Dev-experience
- Better error messages in build.rs [#2173](https://github.com/rerun-io/rerun/pull/2173)
- Recommend sccache in CONTRIBUTING.md [#2245](https://github.com/rerun-io/rerun/pull/2245)
- introduce `re_tracing` [#2283](https://github.com/rerun-io/rerun/pull/2283)
- lint: standardize formatting of let-else-return statements [#2297](https://github.com/rerun-io/rerun/pull/2297)
- Centralized build tools in `re_build_tools` [#2331](https://github.com/rerun-io/rerun/pull/2331)
- Lint for explicit quotes [#2332](https://github.com/rerun-io/rerun/pull/2332)
- Added example screenshot instructions in `just upload --help` [#2454](https://github.com/rerun-io/rerun/pull/2454)
- Added support for puling image from an URL to `upload_image.py` [#2462](https://github.com/rerun-io/rerun/pull/2462)
- `setup_dev.sh` now installs pngcrush [#2470](https://github.com/rerun-io/rerun/pull/2470)
- Added docs/code-examples to the directories checked by py-lint and py-format [#2476](https://github.com/rerun-io/rerun/pull/2476)
- Link to demo in PR + check checkboxes [#2543](https://github.com/rerun-io/rerun/pull/2543)
- Add script to find external issues we haven't commented on [#2532](https://github.com/rerun-io/rerun/pull/2532)
- Move CI-related scripts to its own folder [#2561](https://github.com/rerun-io/rerun/pull/2561)
- Render PR description as template [#2563](https://github.com/rerun-io/rerun/pull/2563)
- Add basic testing automation against all version of Python using nox [#2536](https://github.com/rerun-io/rerun/pull/2536)
- Run clippy on public API too [#2596](https://github.com/rerun-io/rerun/pull/2596)
- Bump all `py-lint`-related package versions [#2600](https://github.com/rerun-io/rerun/pull/2600)
- Crates publishing script [#2604](https://github.com/rerun-io/rerun/pull/2604)
- Fix rust docs deploy [#2615](https://github.com/rerun-io/rerun/pull/2615)
- Add support for .gitignore to scripts/lint.py [#2666](https://github.com/rerun-io/rerun/pull/2666)

#### 🗣 Refactors
- Refactor space-view dependencies:
  - Move spatial space view to its own crate [#2286](https://github.com/rerun-io/rerun/pull/2286)
  - Separate crate for bar chart space view [#2322](https://github.com/rerun-io/rerun/pull/2322)
  - Separate crate for time series space view [#2324](https://github.com/rerun-io/rerun/pull/2324)
  - Separate crate for tensor space view [#2334](https://github.com/rerun-io/rerun/pull/2334)
  - Separate viewport related files out to a new re_viewport crate [#2251](https://github.com/rerun-io/rerun/pull/2251)
  - Remove timepanel dependency from viewport [#2256](https://github.com/rerun-io/rerun/pull/2256)
- New trait system for SpaceViews:
  - Initial Space View trait & port of text space views to the new Space View trait system [#2281](https://github.com/rerun-io/rerun/pull/2281)
  - Extend/iterate on SpaceViewClass framework with SceneContext & port SpatialSpaceView scene parts [#2304](https://github.com/rerun-io/rerun/pull/2304)
  - Finalize move of SpatialSpaceView to SpaceViewClass trait framework [#2311](https://github.com/rerun-io/rerun/pull/2311)
  - Typename cleanup in SpaceViewClass framework [#2321](https://github.com/rerun-io/rerun/pull/2321)
  - Automatic fallback for unrecognized Space View Class, start removing old ViewCategory [#2357](https://github.com/rerun-io/rerun/pull/2357)
  - Rename ScenePart -> ViewPartSystem + related renamings [#2674](https://github.com/rerun-io/rerun/pull/2674)
  - Dynamically registered space view (part/context) systems [#2688](https://github.com/rerun-io/rerun/pull/2688)
- Viewer's command queue is now a channel, allowing to queue commands without mutable access [#2339](https://github.com/rerun-io/rerun/pull/2339)
- Break up app.rs into parts [#2303](https://github.com/rerun-io/rerun/pull/2303)
- Break out `re_log_types::component_types` as `re_components` [#2258](https://github.com/rerun-io/rerun/pull/2258)
- Introduce StoreHub and rename Recording->Store [#2301](https://github.com/rerun-io/rerun/pull/2301)
- Move StoreHub out of the Viewer during Update [#2330](https://github.com/rerun-io/rerun/pull/2330)
- Expand CommandSender to support SystemCommand [#2344](https://github.com/rerun-io/rerun/pull/2344)
- Use `camino` crate for UTF8 paths in `re_types_builder` [#2637](https://github.com/rerun-io/rerun/pull/2637)
- Separate 2d & 3d spaceview classes, removal of `ViewCategory`, `SpaceViewClass` driven spawn heuristics [#2716](https://github.com/rerun-io/rerun/pull/2716)
- Move object property heuristics to heuristics.rs [#2764](https://github.com/rerun-io/rerun/pull/2764)

#### 📦 Dependencies
- Version `rand` & friends at workspace level [#2508](https://github.com/rerun-io/rerun/pull/2508)
- Update to PyO3 0.19 [#2350](https://github.com/rerun-io/rerun/pull/2350)
- Pin `half` to `2.2.1` [#2587](https://github.com/rerun-io/rerun/pull/2587)

#### 📘 Blueprint Changes
- Drive blueprints off of a DataStore [#2010](https://github.com/rerun-io/rerun/pull/2010)
- Split SpaceView -> SpaceViewState + SpaceViewBlueprint [#2188](https://github.com/rerun-io/rerun/pull/2188)
- Split the Blueprint into AppBlueprint and ViewportBlueprint [#2358](https://github.com/rerun-io/rerun/pull/2358)
- Swap the naming of Viewport and ViewportBlueprint [#2595](https://github.com/rerun-io/rerun/pull/2595)
- Basic persistence for blueprints [#2578](https://github.com/rerun-io/rerun/pull/2578)

#### 🏭 New Codegen Framework
- Codegen/IDL 1: add more build tools [#2362](https://github.com/rerun-io/rerun/pull/2362)
- Codegen/IDL 2: introduce `re_types_builder` [#2363](https://github.com/rerun-io/rerun/pull/2363)
- Codegen/IDL 3: introduce `re_types` [#2369](https://github.com/rerun-io/rerun/pull/2369)
- Codegen/IDL 4: definitions for a `Points2D` archetype [#2370](https://github.com/rerun-io/rerun/pull/2370)
- Codegen/IDL 5: auto-generated Python code for `Points2D` [#2374](https://github.com/rerun-io/rerun/pull/2374)
- Codegen/IDL 7: handwritten Python tests and extensions for `Points2D` [#2410](https://github.com/rerun-io/rerun/pull/2410)
- Codegen/IDL 6: auto-generated Rust code for `Points2D` [#2375](https://github.com/rerun-io/rerun/pull/2375)
- Codegen/IDL 8: handwritten Rust tests and extensions for `Points2D` [#2432](https://github.com/rerun-io/rerun/pull/2432)
- Codegen'd Rust/Arrow 1: upgrading to actual `TokenStream`s [#2484](https://github.com/rerun-io/rerun/pull/2484)
- Codegen'd Rust/Arrow 2: matching legacy definitions [#2485](https://github.com/rerun-io/rerun/pull/2485)
- Codegen'd Rust/Arrow 3: misc fixes & improvements [#2487](https://github.com/rerun-io/rerun/pull/2487)
- Codegen'd Rust/Arrow 4: out-of-sync definitions CI detection [#2545](https://github.com/rerun-io/rerun/pull/2545)
- Codegen'd Rust/Arrow 5: doc, definitions and regression tests for combinatorial affixes [#2546](https://github.com/rerun-io/rerun/pull/2546)
- Codegen'd Rust/Arrow 6: serialization [#2549](https://github.com/rerun-io/rerun/pull/2549)
- Codegen'd Rust/Arrow 7: deserialization [#2554](https://github.com/rerun-io/rerun/pull/2554)
- Codegen'd Rust/Arrow 8: carry extension metadata across transparency layers [#2570](https://github.com/rerun-io/rerun/pull/2570)
- Codegen'd Rust/Arrow 9: Rust backport! [#2571](https://github.com/rerun-io/rerun/pull/2571)
- End-to-end cross-language roundtrip tests for our archetypes [#2601](https://github.com/rerun-io/rerun/pull/2601)
- Automatically derive `Debug` and `Clone` in Rust backend [#2613](https://github.com/rerun-io/rerun/pull/2613)
- Generating (de)serialization code for dense unions in Rust backend [#2626](https://github.com/rerun-io/rerun/pull/2626)
- Fix `FixedSizeList` deserialization edge-case + trivial optimizations [#2673](https://github.com/rerun-io/rerun/pull/2673)
- Make `Datatype` & `Component` both inherit from `Loggable` [#2677](https://github.com/rerun-io/rerun/pull/2677)
- Roundtrip-able `Transform3D`s [#2669](https://github.com/rerun-io/rerun/pull/2669)
- Don't inline recursive datatypes in Rust backend [#2760](https://github.com/rerun-io/rerun/pull/2760)
- Automatically derive `tuple_struct` attr and trivial `From` impls where possible [#2772](https://github.com/rerun-io/rerun/pull/2772)
- Introduce roundtrip-able `Points3D` archetype (py + rs) [#2774](https://github.com/rerun-io/rerun/pull/2774)
- Add `fmt::Debug` implementations to various types. [#2784](https://github.com/rerun-io/rerun/pull/2784) (thanks [@kpreid](https://github.com/kpreid)!)
- Isolate testing types in Rust backend [#2810](https://github.com/rerun-io/rerun/pull/2810)
- Fix out-of-sync codegen hash [#2567](https://github.com/rerun-io/rerun/pull/2567)
- Python backport: add `log_any()` [#2581](https://github.com/rerun-io/rerun/pull/2581)
- Integrate unit examples into codegen stack [#2590](https://github.com/rerun-io/rerun/pull/2590)
- Disable codegen on windows [#2592](https://github.com/rerun-io/rerun/pull/2592)
- Python codegen: big cleaning and paving the way towards transforms [#2603](https://github.com/rerun-io/rerun/pull/2603)
- Automatically assume arrow transparency for components [#2608](https://github.com/rerun-io/rerun/pull/2608)
- Fix wrong path being `rerun_if_changed()` in `compute_dir_hash` [#2612](https://github.com/rerun-io/rerun/pull/2612)
- Support transparency at the semantic layer [#2611](https://github.com/rerun-io/rerun/pull/2611)
- Don't use builtin `required` anymore, introduce `nullable` instead [#2619](https://github.com/rerun-io/rerun/pull/2619)
- Rust codegen: generate proper docstrings [#2668](https://github.com/rerun-io/rerun/pull/2668)
- Support nullable Arrow unions using virtual union arms [#2708](https://github.com/rerun-io/rerun/pull/2708)
- Introduce support for querying Archetypes [#2743](https://github.com/rerun-io/rerun/pull/2743)
- Introduce legacy shims and migrate DataCell to re_types::Component [#2752](https://github.com/rerun-io/rerun/pull/2752)

#### 🌊 Starting work on C++
- Seed of C and C++ SDKs [#2594](https://github.com/rerun-io/rerun/pull/2594)
- Move C++ SDK to own folder [#2624](https://github.com/rerun-io/rerun/pull/2624)
- C++ codegen [#2678](https://github.com/rerun-io/rerun/pull/2678)
- C++ codegen for reporting arrow data type for structs [#2756](https://github.com/rerun-io/rerun/pull/2756)
- Don't inline recursive datatypes in C++ backend [#2765](https://github.com/rerun-io/rerun/pull/2765)
- C++ codegen to_arrow_data_type for unions [#2766](https://github.com/rerun-io/rerun/pull/2766)
- C++ codegen arrow serialize non-union components/datatypes without nested rerun types [#2820](https://github.com/rerun-io/rerun/pull/2820)
- C++ codegen of structs and unions [#2707](https://github.com/rerun-io/rerun/pull/2707)
- Fix cpp formatter differences [#2773](https://github.com/rerun-io/rerun/pull/2773)

#### 🤷‍♂️ Other
- test_api: set different app_id based on what test is run [#2599](https://github.com/rerun-io/rerun/pull/2599)
- Introduce `rerun compare` to check whether 2 rrd files are functionally equivalent [#2597](https://github.com/rerun-io/rerun/pull/2597)
- Remove `files.exclude` in vscode settings [#2621](https://github.com/rerun-io/rerun/pull/2621)
- Support feature-gated rust attributes [#2813](https://github.com/rerun-io/rerun/pull/2813)



## [0.7.0](https://github.com/rerun-io/rerun/compare/0.6.0...0.7.0) - improved transforms, better color mapping, bug & doc fixes - 2023-06-16

### Overview & Highlights

While we're working on significant updates around interfaces and customizability,
here's a smaller release packed with useful improvements 🎉

* Much more powerful transformation logging
  * any affine transforms works now!
  * supports many more formats and shows them in the viewer as-is
* Better color mapping range detection for images and tensors
* Many small improvements to samples & documentation

### In detail

#### 🐍 Python SDK
- Improved 3D transform ingestion & affine transform support [#2102](https://github.com/rerun-io/rerun/pull/2102)
- Normalize Python typing syntax to 3.8+ [#2361](https://github.com/rerun-io/rerun/pull/2361)
- Enforce `from __future__ import annotations` in Python files [#2377](https://github.com/rerun-io/rerun/pull/2377)
- Add `jpeg_quality` parameter to `log_image` [#2418](https://github.com/rerun-io/rerun/pull/2418)

#### 🦀 Rust SDK
- Improved 3D transform ingestion & affine transform support [#2102](https://github.com/rerun-io/rerun/pull/2102)
- `impl Copy for Arrow3D`. [#2239](https://github.com/rerun-io/rerun/pull/2239) (thanks [@kpreid](https://github.com/kpreid)!)

#### 🪳 Bug Fixes
- Stable image order, fixing flickering [#2191](https://github.com/rerun-io/rerun/pull/2191)
- Fix double clicking objects no longer focusing the camera on them [#2227](https://github.com/rerun-io/rerun/pull/2227)
- Fix off-by-half pixel error in textured rectangle shader [#2294](https://github.com/rerun-io/rerun/pull/2294)
- Update wgpu-hal to 0.16.1 to fix mobile Safari [#2296](https://github.com/rerun-io/rerun/pull/2296)
- Fix some browsers failing due to 8k texture requirement, pick always highest available now [#2409](https://github.com/rerun-io/rerun/pull/2409)
- Fix visibility toggles for time series not working [#2444](https://github.com/rerun-io/rerun/pull/2444)

#### 🌁 Viewer Improvements
- Time panel now always talks about "events" instead of "messages" [#2247](https://github.com/rerun-io/rerun/pull/2247)
- Automatically determine image/tensor color mapping & need for sRGB decoding [#2342](https://github.com/rerun-io/rerun/pull/2342)

#### 🚀 Performance Improvements
- Optimization: avoid a memory allocation when padding RGB u8 to RGBA [#2345](https://github.com/rerun-io/rerun/pull/2345)

#### 🧑‍🏫 Examples
- Example of how to embed the Rerun Viewer inside your own GUI (+ ergonomic improvements) [#2250](https://github.com/rerun-io/rerun/pull/2250)
- Objectron Rust example: install `protoc` for the user [#2280](https://github.com/rerun-io/rerun/pull/2280)
- Remove weird-looking argument parsing in examples [#2398](https://github.com/rerun-io/rerun/pull/2398)
- Fix `tracking_hf example`: put scaled thing under its own root entity [#2419](https://github.com/rerun-io/rerun/pull/2419)
- Clean up our examples [#2424](https://github.com/rerun-io/rerun/pull/2424)
- New face detection example based on MediaPipe [#2360](https://github.com/rerun-io/rerun/pull/2360)
- Update web examples [#2420](https://github.com/rerun-io/rerun/pull/2420)
- Update titles and tags for examples with real data [#2416](https://github.com/rerun-io/rerun/pull/2416)

#### 📚 Docs
- Merge `rerun-docs` repository into this monorepo [#2284](https://github.com/rerun-io/rerun/pull/2284)
- Add manifest + readmes to examples [#2309](https://github.com/rerun-io/rerun/pull/2309)
- Fix and clean up BUILD.md [#2319](https://github.com/rerun-io/rerun/pull/2319)
- Link to `/examples` in PR description [#2320](https://github.com/rerun-io/rerun/pull/2320)
- Make examples setup a separate page [#2323](https://github.com/rerun-io/rerun/pull/2323)
- Add `site_url` to `mkdocs.yml` [#2326](https://github.com/rerun-io/rerun/pull/2326)
- Add `log_cleared` to the common index [#2400](https://github.com/rerun-io/rerun/pull/2400)
- Use forked `mkdocs-redirects` [#2404](https://github.com/rerun-io/rerun/pull/2404)
- Add support for classes to generated python common API index [#2401](https://github.com/rerun-io/rerun/pull/2401)
- Added support for creating multi-resolution stacks with upload_image.py [#2411](https://github.com/rerun-io/rerun/pull/2411)
- Document annotation context in manual [#2453](https://github.com/rerun-io/rerun/pull/2453)

#### 🕸️ Web
- Update `wasm-bindgen` to 0.2.87 [#2406](https://github.com/rerun-io/rerun/pull/2406)
- When loading on web, match style and show a progress indicator while wasm is loading [#2421](https://github.com/rerun-io/rerun/pull/2421)

#### 📈 Analytics
- Add crash retriever script [#2168](https://github.com/rerun-io/rerun/pull/2168)

#### 🧑‍💻 Dev-experience
- Image uploader script [#2164](https://github.com/rerun-io/rerun/pull/2164)
- Replace `wasm-bindgen-cli` with library `wasm-bindgen-cli-support` [#2257](https://github.com/rerun-io/rerun/pull/2257)
- Fix manual release/dispatch workflows [#2230](https://github.com/rerun-io/rerun/pull/2230)
- Add instructions on how to fix weird `gsutil` crash [#2278](https://github.com/rerun-io/rerun/pull/2278)
- Link to preview of latest commit in PR body [#2287](https://github.com/rerun-io/rerun/pull/2287)
- CI: Retry `linkinator` [#2299](https://github.com/rerun-io/rerun/pull/2299)
- Remove long dead code python unit test [#2356](https://github.com/rerun-io/rerun/pull/2356)
- Added gcloud project name to `upload_image.py` [#2381](https://github.com/rerun-io/rerun/pull/2381)
- Fix typo in `run_all.py` [#2441](https://github.com/rerun-io/rerun/pull/2441)
- Small changelog improvements [#2442](https://github.com/rerun-io/rerun/pull/2442)
- Minor fixes/improvements of `upload_image.py` [#2449](https://github.com/rerun-io/rerun/pull/2449)
- Improve changelog generator [#2447](https://github.com/rerun-io/rerun/pull/2447)

#### 🗣 Refactors
- Centralize freestanding store helpers [#2153](https://github.com/rerun-io/rerun/pull/2153)

#### 📦 Dependencies
- Update `xml-rs` v0.8.13 -> v0.8.14 [#2425](https://github.com/rerun-io/rerun/pull/2425)
- Update pip package `requests` to 2.31 with bug fix [#2426](https://github.com/rerun-io/rerun/pull/2426)


## [0.6.0](https://github.com/rerun-io/rerun/compare/v0.5.1...0.6.0) - 3D in 2D and SDK batching - 2023-05-26

### Overview & Highlights

- You can now show 3D objects in 2D views connected by Pinhole transforms [#2008](https://github.com/rerun-io/rerun/pull/2008)
- You can quickly view images and meshes with `rerun mesh.obj image.png` [#2060](https://github.com/rerun-io/rerun/pull/2060)
- The correct to install the `rerun` binary is now with `cargo install rerun-cli` [#2183](https://github.com/rerun-io/rerun/pull/2183)
- `native_viewer` is now an opt-in feature of the `rerun` library, leading to faster compilation times [#2064](https://github.com/rerun-io/rerun/pull/2064)
- Experimental WebGPU support [#1965](https://github.com/rerun-io/rerun/pull/1965)
- SDK log calls are now batched on the wire, saving CPU time and bandwidth

### In Detail

#### 🐍 Python SDK
- ⚠️ BREAKING: You must now call `rr.init` if you want logging to work.
- ⚠️ BREAKING: `set_enabled` has been removed.
  In order to disable logging at runtime, call `set_global_data_recording(None)`.
  See also [the doc section on this topic](https://www.rerun.io/docs/reference/sdk-logging-controls#dynamically-turn-logging-onoff).
- `log_mesh_file`: accept either path or bytes [#2098](https://github.com/rerun-io/rerun/pull/2098)
- Add `draw_order` to 2D primitives [#2138](https://github.com/rerun-io/rerun/pull/2138)
- Add `rr.version()` [#2084](https://github.com/rerun-io/rerun/pull/2084)
- Add an experimental text-box component and logtype [#2011](https://github.com/rerun-io/rerun/pull/2011)
- Fix a race condition for notebooks [#2073](https://github.com/rerun-io/rerun/pull/2073)
- Redesign multi-recording & multi-threading [#2061](https://github.com/rerun-io/rerun/pull/2061)
- More robust wait for exit condition during `.serve()` [#1939](https://github.com/rerun-io/rerun/pull/1939)
- SDK batching/revamp 3: sunset `PythonSession` [#1985](https://github.com/rerun-io/rerun/pull/1985)

#### 🦀 Rust SDK
- ⚠️ BREAKING: `set_enabled` has been removed.
  In order to disable logging at runtime, create a no-op recording via `RecordingStream::disabled()`.
  See also [the doc section on this topic](https://www.rerun.io/docs/reference/sdk-logging-controls#dynamically-turn-logging-onoff).
- ⚠️ BREAKING: `Session` has been replaced by `RecordingStream` [#1983](https://github.com/rerun-io/rerun/pull/1983)
- ⚠️ BREAKING: `native_viewer` is now an opt-in feature of the `rerun` library [#2064](https://github.com/rerun-io/rerun/pull/2064)
- Rust SDK: bring back support for implicit splats [#2059](https://github.com/rerun-io/rerun/pull/2059)
- Introduce a 2D `DrawOrder` component [#2056](https://github.com/rerun-io/rerun/pull/2056)
- Add `Tensor::from_image_file` and `Tensor::from_image_bytes` [#2097](https://github.com/rerun-io/rerun/pull/2097)
- Redesign multi-recording & multi-threading [#2061](https://github.com/rerun-io/rerun/pull/2061)

#### 🌁 Viewer Improvements
- Support projecting 3D entities in 2D views [#2008](https://github.com/rerun-io/rerun/pull/2008)
- Set Rerun viewer native app icon using eframe [#1976](https://github.com/rerun-io/rerun/pull/1976)
- Use `alt` key again for rolling camera in 3d views [#2066](https://github.com/rerun-io/rerun/pull/2066)
- Show tensors shaped [H, W, 1, 1] as images (and more!) [#2075](https://github.com/rerun-io/rerun/pull/2075)
- Show meshes and images with `rerun foo.obj bar.png` [#2060](https://github.com/rerun-io/rerun/pull/2060)
- Don't persist blueprints for unknown apps [#2165](https://github.com/rerun-io/rerun/pull/2165)

#### 🪳 Bug Fixes
- Fix hover/select highlights when picking single points in a scene with multiple point clouds [#1942](https://github.com/rerun-io/rerun/pull/1942)
- Fix crash for missing class ids causing zero sized texture [#1947](https://github.com/rerun-io/rerun/pull/1947)
- Handle leaking of prerelease into alpha version [#1953](https://github.com/rerun-io/rerun/pull/1953)
- Fix incorrect memory usage stats for destroyed on-creation-mapped buffers [#1963](https://github.com/rerun-io/rerun/pull/1963)
- Fix: don't starve web-socket decoding task [#1977](https://github.com/rerun-io/rerun/pull/1977)
- When hovering a 3D view in the presence of images, fix previously incorrect depth shown in 2D view [#2009](https://github.com/rerun-io/rerun/pull/2009)
- Fix: use the mac icon on mac [#2023](https://github.com/rerun-io/rerun/pull/2023)
- SDK batching/revamp 2.2: homegrown arrow size estimation routines [#2002](https://github.com/rerun-io/rerun/pull/2002)
- Fix twice as wide alpha-to-coverage edge on circles, leading to artifacts [#2053](https://github.com/rerun-io/rerun/pull/2053)
- Bugfix: allow hovered items to be clicked to set selection [#2057](https://github.com/rerun-io/rerun/pull/2057)
- Detect, warn and gracefully handle corrupt cells in `lookup_arrow` [#2055](https://github.com/rerun-io/rerun/pull/2055)
- Fix failing dependency install of mesh_to_sdf [#2081](https://github.com/rerun-io/rerun/pull/2081)
- Stop playback when we reach the end of the data [#2085](https://github.com/rerun-io/rerun/pull/2085)
- `tornado` >6.1 doesn't work with recent `jupyter` [#2092](https://github.com/rerun-io/rerun/pull/2092)
- Premultiply alpha of RGBA u8 images [#2095](https://github.com/rerun-io/rerun/pull/2095)
- Fix premature pausing when reaching end of still-streaming stream [#2106](https://github.com/rerun-io/rerun/pull/2106)
- 2D layering fixes [#2080](https://github.com/rerun-io/rerun/pull/2080)
- Fix depth precision issues on WebGL due to different NDC space [#2123](https://github.com/rerun-io/rerun/pull/2123)
- Fix flushing race in new multi-recording SDK [#2125](https://github.com/rerun-io/rerun/pull/2125)
- Web viewer: catch and show panic messages that happens at startup [#2157](https://github.com/rerun-io/rerun/pull/2157)
- Don't early-exit on non-pinhole transforms when looking up cameras [#2194](https://github.com/rerun-io/rerun/pull/2194)
- Mitigate depth offset precision issues on web [#2187](https://github.com/rerun-io/rerun/pull/2187)
- Fix colormaps [#2204](https://github.com/rerun-io/rerun/pull/2204)
- Fix annotation images sometimes drawn in the background [#1933](https://github.com/rerun-io/rerun/pull/1933)
- Fix hovering depth clouds [#1943](https://github.com/rerun-io/rerun/pull/1943)
- Fix incorrect 2D camera for scenes with negative 2D coordinates [#2051](https://github.com/rerun-io/rerun/pull/2051)
- Fix web depth/projection regression, causing incorrect rendering on all 3D scenes [#2170](https://github.com/rerun-io/rerun/pull/2170)

#### 🚀 Performance Improvements
- SDK batching/revamp 1: impl `DataTableBatcher` [#1980](https://github.com/rerun-io/rerun/pull/1980)
- Upgrade arrow2/convert and use native buffers for the tensor u8 types [#1375](https://github.com/rerun-io/rerun/pull/1375)
- Use the same RRD encoding for the SDK comms as for everything else [#2065](https://github.com/rerun-io/rerun/pull/2065)
- Optimize GLTF/GLB texture loading in debug builds [#2096](https://github.com/rerun-io/rerun/pull/2096)
- Premultiply the alpha on the GPU [#2190](https://github.com/rerun-io/rerun/pull/2190)
- Switch compression algorithm from zstd to lz4 [#2112](https://github.com/rerun-io/rerun/pull/2112)
- Support RRD streams with and without compression. Turn off for SDK comms [#2219](https://github.com/rerun-io/rerun/pull/2219)

#### 🧑‍🏫 Examples
- Join threads at end of multi-threading example [#1934](https://github.com/rerun-io/rerun/pull/1934)
- Add argument parsing to the rerun_demo [#1925](https://github.com/rerun-io/rerun/pull/1925)
- Use zipfile python library instead of `unzip` command in arkitscene [#1936](https://github.com/rerun-io/rerun/pull/1936)
- Fix backslashes in arkitscene rigid transformation path [#1938](https://github.com/rerun-io/rerun/pull/1938)
- Fix mp_pose example 2D points having incorrectly interpreted depth [#2034](https://github.com/rerun-io/rerun/pull/2034)
- SDK batching/revamp 2.1: `clock` example for Rust [#2000](https://github.com/rerun-io/rerun/pull/2000)
- Add `scripts/run_all.py` [#2046](https://github.com/rerun-io/rerun/pull/2046)
- Check `examples/python/requirements.txt` in CI [#2063](https://github.com/rerun-io/rerun/pull/2063)
- Fix glb mesh data set downloads [#2100](https://github.com/rerun-io/rerun/pull/2100)
- Add more examples to https://app.rerun.io/ [#2062](https://github.com/rerun-io/rerun/pull/2062)

#### 🖼 UI Improvements
- Update egui to latest and wgpu to 0.16 [#1958](https://github.com/rerun-io/rerun/pull/1958)
- Add keyboard shortcut for "Follow", and stop following on "Restart" [#1986](https://github.com/rerun-io/rerun/pull/1986) (thanks [@h3mosphere](https://github.com/h3mosphere)!)
- Improve UI for keypoint and class-ids of annotations contexts [#2071](https://github.com/rerun-io/rerun/pull/2071)
- Improvements to memory measurements and reporting [#2069](https://github.com/rerun-io/rerun/pull/2069)
- Switch from `egui_dock` to `egui_tiles` [#2082](https://github.com/rerun-io/rerun/pull/2082)
- Allow horizontal scrolling in blueprint panel [#2114](https://github.com/rerun-io/rerun/pull/2114)
- Nicer (& fixed up) help texts for space views [#2070](https://github.com/rerun-io/rerun/pull/2070)
- Allow dragging time cursor in plots [#2115](https://github.com/rerun-io/rerun/pull/2115)

#### 🕸️ Web
- Set the GC limit to 2.5GB on web [#1944](https://github.com/rerun-io/rerun/pull/1944)
- Better crash reports on Web, plus WebGPU support detection [#1975](https://github.com/rerun-io/rerun/pull/1975)
- Work around https://github.com/sebcrozet/instant/issues/49 [#2094](https://github.com/rerun-io/rerun/pull/2094)
- Update `wasm-bindgen` to 0.2.86 [#2161](https://github.com/rerun-io/rerun/pull/2161)

#### 🎨 Renderer Improvements
- Full (experimental) WebGPU support [#1965](https://github.com/rerun-io/rerun/pull/1965)
- Depth offset for lines & points [#2052](https://github.com/rerun-io/rerun/pull/2052)
- Update to wgpu 0.16.1 [#2205](https://github.com/rerun-io/rerun/pull/2205)

#### 🚜 Refactors
- Replace complex uses of `query_entity_with_primary` with `query_latest_single` [#2137](https://github.com/rerun-io/rerun/pull/2137)
- Make selection state independent of blueprint [#2035](https://github.com/rerun-io/rerun/pull/2035)
- Remove unused MeshSourceData [#2036](https://github.com/rerun-io/rerun/pull/2036)
- Move selection state into an independent crate, re_viewer_context [#2037](https://github.com/rerun-io/rerun/pull/2037)
- Move item-ui to separate module, move AppOptions to re_viewer_context [#2040](https://github.com/rerun-io/rerun/pull/2040)
- Move `Caches` to `re_viewer_ctx` and make it generic [#2043](https://github.com/rerun-io/rerun/pull/2043)
- Move time control to re_viewer_context [#2045](https://github.com/rerun-io/rerun/pull/2045)
- Move `ViewerContext` & `ComponentUiRegistry` to `viewer_context` [#2047](https://github.com/rerun-io/rerun/pull/2047)
- Move data ui to new `re_data_ui` crate [#2048](https://github.com/rerun-io/rerun/pull/2048)
- Use instant for `Time::now()` [#2090](https://github.com/rerun-io/rerun/pull/2090)
- Move from `instant` -> `web_time` [#2093](https://github.com/rerun-io/rerun/pull/2093)
- "namespace" flag parameters for linestrip & point cloud shader flags [#2033](https://github.com/rerun-io/rerun/pull/2033)

#### ✨ Other Enhancement
- Update minimum supported Rust version to `1.69.0` [#1935](https://github.com/rerun-io/rerun/pull/1935)
- Allow users to select the bind address (ip) to use with `--bind` [#2159](https://github.com/rerun-io/rerun/pull/2159)

#### 🧑‍💻 Dev-experience
- Suggest users open an issue on crash, and other fixes [#1993](https://github.com/rerun-io/rerun/pull/1993)
- Lint error names in `map_err` [#1948](https://github.com/rerun-io/rerun/pull/1948)
- New dispatch-only workflow for running the lint-job [#1950](https://github.com/rerun-io/rerun/pull/1950)
- Move clippy_wasm/clippy.toml to under scripts [#1949](https://github.com/rerun-io/rerun/pull/1949)
- Fix run-wasm crash on trying to wait for server [#1959](https://github.com/rerun-io/rerun/pull/1959)
- Introduce new reusable workflow jobs and cleanup manual trigger [#1954](https://github.com/rerun-io/rerun/pull/1954)
- Use new CI workflows on pull-request [#1955](https://github.com/rerun-io/rerun/pull/1955)
- Try making pull-request workflows non-concurrent [#1970](https://github.com/rerun-io/rerun/pull/1970)
- Another attempt to make jobs non-concurrent on a per-PR basis [#1974](https://github.com/rerun-io/rerun/pull/1974)
- If there's a `{{ pr-build-summary }}` in the PR description, update it. [#1971](https://github.com/rerun-io/rerun/pull/1971)
- Run the cube notebook on PR [#1972](https://github.com/rerun-io/rerun/pull/1972)
- Add ability to manually run a web build to upload to an adhoc name [#1966](https://github.com/rerun-io/rerun/pull/1966)
- Limit ipython to 8.12 in the jupyter example [#2001](https://github.com/rerun-io/rerun/pull/2001)
- New manual job to publish a release based on pre-built wheels [#2025](https://github.com/rerun-io/rerun/pull/2025)
- Use the correct rust analyzer settings [#2028](https://github.com/rerun-io/rerun/pull/2028)
- New helper for sticking Serde-encodable data into arrow [#2004](https://github.com/rerun-io/rerun/pull/2004)
- Fix `taplo-cli` failing to install [#2068](https://github.com/rerun-io/rerun/pull/2068)
- `run_all.py`: add `--fast`, `--separate`, and `--close` [#2054](https://github.com/rerun-io/rerun/pull/2054)
- Remove `Clipboard::set_text` [#2078](https://github.com/rerun-io/rerun/pull/2078)
- run_all.py: print output on sequential run failure [#2079](https://github.com/rerun-io/rerun/pull/2079)
- Use the american spelling of "gray" [#2099](https://github.com/rerun-io/rerun/pull/2099)
- Make sure `rerun/rerun_py/re_viewer` build info is updated on each build [#2087](https://github.com/rerun-io/rerun/pull/2087)
- Fix setup scripts for Mac M1/MacPort configuration [#2169](https://github.com/rerun-io/rerun/pull/2169) (thanks [@abey79](https://github.com/abey79)!)
- Better error messages in `build.rs` [#2173](https://github.com/rerun-io/rerun/pull/2173)
- `cargo install rerun-cli` [#2183](https://github.com/rerun-io/rerun/pull/2183)
- Fix `cargo test` [#2199](https://github.com/rerun-io/rerun/pull/2199)
- Fix run all for new rust-cli target & add rerun-web alias for quick running of the web player [#2203](https://github.com/rerun-io/rerun/pull/2203)

#### 🤷‍♂️ Other
- Fix secret in dispatch_lint.yml [4848f98f2605a3caf9b7695273e0871efa2d44c8](https://github.com/rerun-io/rerun/commit/4848f98f2605a3caf9b7695273e0871efa2d44c8)
- Only maintain a single manual-dispatch job for testing workflows [98f7de3b52b0fea6abe364f9d0ce0bd4c459caf1](https://github.com/rerun-io/rerun/commit/98f7de3b52b0fea6abe364f9d0ce0bd4c459caf1)
- Add other build parametrizations to manual_dispatch.yml [dbdf275eaf17220d14811dc34b69b6a76e948e73](https://github.com/rerun-io/rerun/commit/dbdf275eaf17220d14811dc34b69b6a76e948e73)
- Use proper if gates on the manual_dispatch.yml jobs [9ad62011678caaed04260ba160763e24e64a7402](https://github.com/rerun-io/rerun/commit/9ad62011678caaed04260ba160763e24e64a7402)
- Add ability to save cache to manual_dispatch.yml [5c61b37a1bc40f1a223c370b3b69b08654aada47](https://github.com/rerun-io/rerun/commit/5c61b37a1bc40f1a223c370b3b69b08654aada47)
- Standard case of inputs [2729c71f1ba9f7cdbe64adc3c610caf9464324e4](https://github.com/rerun-io/rerun/commit/2729c71f1ba9f7cdbe64adc3c610caf9464324e4)
- Add manual step for packaging to 'manual_dispatch.yml' [a3178e6143c068175b477cb236f2ba2477e083ea](https://github.com/rerun-io/rerun/commit/a3178e6143c068175b477cb236f2ba2477e083ea)
- New workflow_dispatch for building wheels for a PR [3bc2cb73ece98f914254221ce0ea129015834f59](https://github.com/rerun-io/rerun/commit/3bc2cb73ece98f914254221ce0ea129015834f59)
- Rename build_wheels_for_pr.yml -> manual_build_wheels_for_pr.yml [778c4d363b3814aeb777d07bfa63f081bc1dac32](https://github.com/rerun-io/rerun/commit/778c4d363b3814aeb777d07bfa63f081bc1dac32)
- New manual workflow for running benches [840a127e3a74c3520a27c0b19eb1d3d9a7255b07](https://github.com/rerun-io/rerun/commit/840a127e3a74c3520a27c0b19eb1d3d9a7255b07)
- New manual workflow for adhoc web builds [01080d6509e94fd2e2d3c4ff05beb0970ebe0b6e](https://github.com/rerun-io/rerun/commit/01080d6509e94fd2e2d3c4ff05beb0970ebe0b6e)
- Fix name of on_push_main.yml [bf5f63344663b3ebfc74f847db696a749b3e716c](https://github.com/rerun-io/rerun/commit/bf5f63344663b3ebfc74f847db696a749b3e716c)
- Fix usage of long commit in generate_prerelease_pip_index.py [579ce91556d6dd3cb9e6bd46971a7b6db6e42cdd](https://github.com/rerun-io/rerun/commit/579ce91556d6dd3cb9e6bd46971a7b6db6e42cdd)
- Jobs with duplicated instances still need separate concurrency keys based on platform [0ad19980be99cb2f669d38c2f1410a38206cbe74](https://github.com/rerun-io/rerun/commit/0ad19980be99cb2f669d38c2f1410a38206cbe74)
- New manual CI job for creating a release [fb2d41af5ec089f6c7583629eda3fb332e420488](https://github.com/rerun-io/rerun/commit/fb2d41af5ec089f6c7583629eda3fb332e420488)
- Version check needs to run in bash [6feca463d21ea03538889df08064b6974edb1fd2](https://github.com/rerun-io/rerun/commit/6feca463d21ea03538889df08064b6974edb1fd2)
- Update changelog with 0.5.1 release notes [40fc2fd7d61689100dc40bfe59e4ddfbcc819c7d](https://github.com/rerun-io/rerun/commit/40fc2fd7d61689100dc40bfe59e4ddfbcc819c7d)
- `RecordingStream`: automatic `log_tick` timeline [#2072](https://github.com/rerun-io/rerun/pull/2072)
- Add support for `f16` tensors [#1449](https://github.com/rerun-io/rerun/pull/1449)
- Make `RecordingId` a string [#2088](https://github.com/rerun-io/rerun/pull/2088)
- Update to latest `egui_tiles` [#2091](https://github.com/rerun-io/rerun/pull/2091)
- Make every `RecordingId` typed and preclude the existence of 'Defaults' [#2110](https://github.com/rerun-io/rerun/pull/2110)
- Add unit test of `re_smart_channel` `is_connected` [#2119](https://github.com/rerun-io/rerun/pull/2119)
- `BeingRecordingMsg` -> `SetRecordingInfo` [#2149](https://github.com/rerun-io/rerun/pull/2149)
- Update egui and eframe [#2184](https://github.com/rerun-io/rerun/pull/2184)
- Update to egui 0.22 [#2195](https://github.com/rerun-io/rerun/pull/2195)
- Simpler SIGINT handling [#2198](https://github.com/rerun-io/rerun/pull/2198)
- `cargo update` [#2196](https://github.com/rerun-io/rerun/pull/2196)
- Replace `ctrlc` crate with `tokio` [#2207](https://github.com/rerun-io/rerun/pull/2207)
- Comment indicating blueprints aren't available in 0.6 [b6c05776ab48e759370d6fed645ffd0ea68ec8c0](https://github.com/rerun-io/rerun/commit/b6c05776ab48e759370d6fed645ffd0ea68ec8c0)


## [0.5.1](https://github.com/rerun-io/rerun/compare/v0.5.1...v0.5.0) - Patch Release - 2023-05-01

### Overview & Highlights
This Release fixes a few small bugs on top of the v0.5.0 release.

### In Detail
* Bump hyper version due to RUSTSEC-2023-0034 [#1951](https://github.com/rerun-io/rerun/pull/1951)
* Round to nearest color_index when doing color mapping [#1969](https://github.com/rerun-io/rerun/pull/1969)
* Use an sRGB-correct gray gradient when displaying grayscale images [#2014](https://github.com/rerun-io/rerun/pull/2014)
* Don't use console.error [#1984](https://github.com/rerun-io/rerun/pull/1984)
* Fix failure to save files when split table contains no data [#2007](https://github.com/rerun-io/rerun/pull/2007)


## [0.5.0](https://github.com/rerun-io/rerun/compare/v0.4.0...v0.5.0) - Jupyter MVP, GPU-based picking & colormapping, new datastore! - 2023-04-20

### Overview & Highlights

This new release adds MVP support for embedding Rerun in Jupyter notebooks, and brings significant performance improvements across all layers of the stack.

* Rerun can now be embedded in Jupyter notebooks
    * Tested with Jupyter Notebook Classic, Jupyter Lab, VSCode & Google Colab; checkout our [How-to guide](https://www.rerun.io/docs/howto/notebook)
    * Try it out live on [Google Colab](https://colab.research.google.com/drive/1R9I7s4o6wydQC_zkybqaSRFTtlEaked_?usp=sharing)
* All colormapping tasks are now done directly on the GPU
    * This yields _very significant_ performance improvements for colormapping heavy workload (e.g. segmentation)
    * Try it out in our new [`segment_anything` example](https://www.rerun.io/examples/real-data/segment_anything_model) that shows off the latest models from Meta AI
* GPU picking & hovering now works with all of our primitives, including meshes & depth clouds
    * This fixes all the shortcomings of the previous CPU-based system
    * Rerun's automatic backprojection of depth textures ("depth clouds") is now feature complete
    * Try it out in our updated [`nyud` example](https://www.rerun.io/examples/real-data/rgbd)
* Our datastore has been completely revamped to more closely match our latest data model
    * This yields _very significant_ performance improvements for workloads with many events
    * Checkout [this post](https://github.com/rerun-io/rerun/issues/1619#issuecomment-1511046649) for a detailed walkthrough of the changes

### In Detail

#### 🐍 Python SDK
- Document that we also accept colors in 0-1 floats [#1740](https://github.com/rerun-io/rerun/pull/1740)
- Don't initialize an SDK session if we are only going to be launching the app [#1768](https://github.com/rerun-io/rerun/pull/1768)
- Allow torch tensors for `log_rigid3` [#1769](https://github.com/rerun-io/rerun/pull/1769)
- Always send `recording_id` as part of `LogMsg` [#1778](https://github.com/rerun-io/rerun/pull/1778)
- New `reset_time` API [#1826](https://github.com/rerun-io/rerun/pull/1826) [#1854](https://github.com/rerun-io/rerun/pull/1854)
- Always flush when we remove a sink [#1830](https://github.com/rerun-io/rerun/pull/1830)
- More robust wait for exit condition during .serve() [#1939](https://github.com/rerun-io/rerun/pull/1939)

#### 🪳 Bug Fixes
- Fix broken outlines (hover/select effect) for lines [#1724](https://github.com/rerun-io/rerun/pull/1724)
- Fix logged obb being displayed with half of the requested size [#1749](https://github.com/rerun-io/rerun/pull/1749) (thanks [@BenjaminDev](https://github.com/BenjaminDev)!)
- Fix `log_obb` usage [#1761](https://github.com/rerun-io/rerun/pull/1761)
- Always create the `log_time` timeline [#1763](https://github.com/rerun-io/rerun/pull/1763)
- Fix undo/redo selection shortcut/action changing selection history without changing selection [#1765](https://github.com/rerun-io/rerun/pull/1765)
- Fix various crashes [#1780](https://github.com/rerun-io/rerun/pull/1780)
- Fix crash when trying to do picking on depth clouds [d94ca3dd35e73e1984ccb969d0c7abd0d3e0faa9](https://github.com/rerun-io/rerun/commit/d94ca3dd35e73e1984ccb969d0c7abd0d3e0faa9)
- ci: fix benchmarks [#1799](https://github.com/rerun-io/rerun/pull/1799)
- ci: fix `cargo deny` [#1806](https://github.com/rerun-io/rerun/pull/1806)
- Fix "too many points" crash [#1822](https://github.com/rerun-io/rerun/pull/1822)
- Allow re-use of `RowId`s if no conflict is possible [#1832](https://github.com/rerun-io/rerun/pull/1832)
- Reduce memory used by staging belts on Web [#1836](https://github.com/rerun-io/rerun/pull/1836)
- Test and handle all tensor dtypes as images [#1840](https://github.com/rerun-io/rerun/pull/1840)
- Fix the python build when running without `web_viewer` enabled [#1856](https://github.com/rerun-io/rerun/pull/1856)
- Error instead of `expect` inside `msg_encode` [#1857](https://github.com/rerun-io/rerun/pull/1857)
- Fix shutdown race condition in `re_sdk_comms` client [#1861](https://github.com/rerun-io/rerun/pull/1861)
- Fix broken instance picking in presence of images [#1876](https://github.com/rerun-io/rerun/pull/1876)
- Make sure JPEGs are always decoded [#1884](https://github.com/rerun-io/rerun/pull/1884)
- Fix crash when saving store to file [#1909](https://github.com/rerun-io/rerun/pull/1909)
- Don't clean up `LogDb`s that only contain a `BeginRecordingMsg` [#1914](https://github.com/rerun-io/rerun/pull/1914)
- Fix picking entities with image + another object (or label) twice [#1908](https://github.com/rerun-io/rerun/pull/1908)
- Fix double clicking camera no longer focusing on said camera [#1911](https://github.com/rerun-io/rerun/pull/1911)
- Fix annotation images sometimes drawn in the background [#1933](https://github.com/rerun-io/rerun/pull/1933)
- Use `zipfile` python library instead of `unzip` command in `arkitscene` demo [#1936](https://github.com/rerun-io/rerun/pull/1936)
- Fix backslashes in `arkitscene` rigid transformation path [#1938](https://github.com/rerun-io/rerun/pull/1938)
- Fix hover/select highlights when picking single points in a scene with multiple point clouds [#1942](https://github.com/rerun-io/rerun/pull/1942)
- Fix hovering depth clouds [#1943](https://github.com/rerun-io/rerun/pull/1943)

#### 🚀 Performance Improvements
- batching 4: retire `MsgBundle` + batching support in transport layer [#1679](https://github.com/rerun-io/rerun/pull/1679)
- Optimize the depth-cloud shader when `depth=0` [#1729](https://github.com/rerun-io/rerun/pull/1729)
- `arrow2_convert` primitive (de)serialization benchmarks [#1742](https://github.com/rerun-io/rerun/pull/1742)
- `arrow2` `estimated_bytes_size` benchmarks [#1743](https://github.com/rerun-io/rerun/pull/1743)
- `arrow2` erased refcounted clones benchmarks [#1745](https://github.com/rerun-io/rerun/pull/1745)
- benchmarks for common vector ops across `smallvec`/`tinyvec`/std [#1747](https://github.com/rerun-io/rerun/pull/1747)
- Columnar `TimePoint`s in data tables and during transport [#1767](https://github.com/rerun-io/rerun/pull/1767)
- Compile with `panic = "abort"` [#1813](https://github.com/rerun-io/rerun/pull/1813)
- Process 2D points per entities like 3D points [#1820](https://github.com/rerun-io/rerun/pull/1820)
- re_query: use latest data types (`DataRow`/`DataCell`) [#1828](https://github.com/rerun-io/rerun/pull/1828)
- Depth cloud textures are now cached frame-to-frame [#1913](https://github.com/rerun-io/rerun/pull/1913)

#### 🧑‍🏫 Examples
- Add new `ARKitScenes` example [#1538](https://github.com/rerun-io/rerun/pull/1538) (thanks [@pablovela5620](https://github.com/pablovela5620)!)
- New example code for Facebook research's `segment-anything` [#1788](https://github.com/rerun-io/rerun/pull/1788)
- Add `minimal_options` example for Rust SDK [#1773](https://github.com/rerun-io/rerun/pull/1773) (thanks [@h3mosphere](https://github.com/h3mosphere)!)
- Remove manual depth projection from `car` and `nyud` examples [#1869](https://github.com/rerun-io/rerun/pull/1869)
- Always spawn instead of fork in multiprocessing example [#1922](https://github.com/rerun-io/rerun/pull/1922)
- Add `--num-frames` arg to canny (webcam) example [#1923](https://github.com/rerun-io/rerun/pull/1923)
- Add argument parsing to `rerun_demo` [#1925](https://github.com/rerun-io/rerun/pull/1925)
- Join threads at end of `multithreading` example [#1934](https://github.com/rerun-io/rerun/pull/1934)

#### 📚 Docs
- Add `typing_extensions` to `requirements-doc.txt` [#1786](https://github.com/rerun-io/rerun/pull/1786)
- Fix typos in notebook readme [#1852](https://github.com/rerun-io/rerun/pull/1852)
- Update docs related to notebook [#1915](https://github.com/rerun-io/rerun/pull/1915)

#### 🖼 UI Improvements
- Hover rays for tracked 3D cameras [#1751](https://github.com/rerun-io/rerun/pull/1751)
- Collapse space-view by default if there is only one child [#1762](https://github.com/rerun-io/rerun/pull/1762)
- Option to show scene bounding box [#1770](https://github.com/rerun-io/rerun/pull/1770)
- Assign default colors to class-ids when annotation context is missing [#1783](https://github.com/rerun-io/rerun/pull/1783)
- Add Restart command and keyboard shortcut for moving time to start of timeline [#1802](https://github.com/rerun-io/rerun/pull/1802) (thanks [@h3mosphere](https://github.com/h3mosphere)!)
- New option to disable persistent storage [#1825](https://github.com/rerun-io/rerun/pull/1825)
- Show previews of colormaps when selecting them [#1846](https://github.com/rerun-io/rerun/pull/1846)
- Smooth out scroll wheel input for camera zooming [#1920](https://github.com/rerun-io/rerun/pull/1920)

#### 🤷‍♂️ Other Viewer Improvements
- Change `EntityPathHash` to be 64 bit [#1723](https://github.com/rerun-io/rerun/pull/1723)
- Central `GpuReadback` handling for re_viewer, experimental space view screenshots [#1717](https://github.com/rerun-io/rerun/pull/1717)
- Readback depth from GPU picking [#1752](https://github.com/rerun-io/rerun/pull/1752)
- Use GPU picking for points, streamline/share picking code some more [#1814](https://github.com/rerun-io/rerun/pull/1814)
- Use GPU picking for line(like) primitives, fix `interactive` flags [#1829](https://github.com/rerun-io/rerun/pull/1829)
- Use GPU colormapping when showing images in the GUI [#1865](https://github.com/rerun-io/rerun/pull/1865)

#### 🕸️ Web
- Make CI publish `latest` tagged web-viewer to `app.rerun.io` [#1725](https://github.com/rerun-io/rerun/pull/1725)
- Implement `re_tuid::Tuid::random()` on web [#1796](https://github.com/rerun-io/rerun/pull/1796)
- Refactor the relationship between the assorted web / websocket servers [#1844](https://github.com/rerun-io/rerun/pull/1844)
- Notebooks: make `presentation_id` consistent and use data-attribute for rrd [#1881](https://github.com/rerun-io/rerun/pull/1881)
- 2.5GB before GC kick in on web [#1944](https://github.com/rerun-io/rerun/pull/1944)

#### 🎨 Renderer Improvements
- GPU based picking with points [#1721](https://github.com/rerun-io/rerun/pull/1721)
- improved renderer label handling [#1731](https://github.com/rerun-io/rerun/pull/1731)
- Improved readback data handling [#1734](https://github.com/rerun-io/rerun/pull/1734)
- GPU based mesh picking [#1737](https://github.com/rerun-io/rerun/pull/1737)
- Improve dealing with raw buffers for texture read/write [#1744](https://github.com/rerun-io/rerun/pull/1744)
- GPU colormapping, first step [#1835](https://github.com/rerun-io/rerun/pull/1835)
- GPU tensor colormapping [#1841](https://github.com/rerun-io/rerun/pull/1841)
- GPU picking for depth clouds [#1849](https://github.com/rerun-io/rerun/pull/1849)
- Implement bilinear filtering of textures [#1850](https://github.com/rerun-io/rerun/pull/1850) [#1859](https://github.com/rerun-io/rerun/pull/1859) [#1860](https://github.com/rerun-io/rerun/pull/1860)
- Refactor: remove `GpuTexture2DHandle::invalid` [#1866](https://github.com/rerun-io/rerun/pull/1866)
- Fix filtering artifact for non-color images [#1886](https://github.com/rerun-io/rerun/pull/1886)
- Refactor: Add helper functions to `GpuTexture2DHandle` [#1900](https://github.com/rerun-io/rerun/pull/1900)

#### 🛢 Datastore Improvements
- Datastore: revamp bench suite [#1733](https://github.com/rerun-io/rerun/pull/1733)
- Datastore revamp 1: new indexing model & core datastructures [#1727](https://github.com/rerun-io/rerun/pull/1727)
- Datastore revamp 2: serialization & formatting [#1735](https://github.com/rerun-io/rerun/pull/1735)
- Datastore revamp 3: efficient incremental stats [#1739](https://github.com/rerun-io/rerun/pull/1739)
- Datastore revamp 4: sunset `MsgId` [#1785](https://github.com/rerun-io/rerun/pull/1785)
- Datastore revamp 5: `DataStore::to_data_tables()` [#1791](https://github.com/rerun-io/rerun/pull/1791)
- Datastore revamp 6: sunset `LogMsg` storage + save store to disk [#1795](https://github.com/rerun-io/rerun/pull/1795)
- Datastore revamp 7: garbage collection [#1801](https://github.com/rerun-io/rerun/pull/1801)
- Incremental metadata registry stats [#1833](https://github.com/rerun-io/rerun/pull/1833)

#### 🗣 Merged RFCs
- RFC: datastore state of the union & end-to-end batching [#1610](https://github.com/rerun-io/rerun/pull/1610)

#### 🧑‍💻 Dev-experience
- Post-release cleanup [#1726](https://github.com/rerun-io/rerun/pull/1726)
- Remove unnecessary dependencies [#1711](https://github.com/rerun-io/rerun/pull/1711) (thanks [@vsuryamurthy](https://github.com/vsuryamurthy)!)
- Use copilot markers in PR template [#1784](https://github.com/rerun-io/rerun/pull/1784)
- re_format: barebone support for custom formatting [#1776](https://github.com/rerun-io/rerun/pull/1776)
- Refactor: Add new helper crate `re_log_encoding` [#1772](https://github.com/rerun-io/rerun/pull/1772)
- `setup_web.sh` supports pacman package manager [#1797](https://github.com/rerun-io/rerun/pull/1797) (thanks [@urholaukkarinen](https://github.com/urholaukkarinen)!)
- Add `rerun --strict`: crash if any warning or error is logged [#1812](https://github.com/rerun-io/rerun/pull/1812)
- End-to-end testing of python logging -> store ingestion [#1817](https://github.com/rerun-io/rerun/pull/1817)
- Fix e2e test on CI: Don't try to re-build `rerun-sdk` [#1821](https://github.com/rerun-io/rerun/pull/1821)
- Install the rerun-sdk in CI using `--no-index` and split out linux wheel build to run first [#1838](https://github.com/rerun-io/rerun/pull/1838)
- Remove more unused dependencies [#1863](https://github.com/rerun-io/rerun/pull/1863)
- Improve end-to-end testing slightly [#1862](https://github.com/rerun-io/rerun/pull/1862)
- Turn off benchmarks comment in each PR [#1872](https://github.com/rerun-io/rerun/pull/1872)
- Fix double-negation in `scripts/run_python_e2e_test.py` [#1896](https://github.com/rerun-io/rerun/pull/1896)
- Improve PR template with better comment, and no copilot by default [#1901](https://github.com/rerun-io/rerun/pull/1901)
- Optimize `generate_changelog.py` [#1912](https://github.com/rerun-io/rerun/pull/1912)

#### 🤷‍♂️ Other
- Fix videos for GitHub in `CHANGELOG.md` [af7d3b192157f942e35f64d3561a9a8dbcc18bfa](https://github.com/rerun-io/rerun/commit/af7d3b192157f942e35f64d3561a9a8dbcc18bfa)
- Don't run 3rd party bench suites on CI [#1787](https://github.com/rerun-io/rerun/pull/1787)
- Remove `TensorTrait` [#1819](https://github.com/rerun-io/rerun/pull/1819)
- Disable wheel tests for `x86_64-apple-darwin` [#1853](https://github.com/rerun-io/rerun/pull/1853)
- Update `enumflags2` to non-yanked version [#1874](https://github.com/rerun-io/rerun/pull/1874)
- Collect extra egui features into the main `Cargo.toml` [#1926](https://github.com/rerun-io/rerun/pull/1926)
- `just rs-run-all` [b14087b40bd805c95f030a4c7d3fb7a0482e13f4](https://github.com/rerun-io/rerun/commit/b14087b40bd805c95f030a4c7d3fb7a0482e13f4)
- `just py-run-all-{native|web|rrd}` [#1927](https://github.com/rerun-io/rerun/pull/1927)


## [0.4.0](https://github.com/rerun-io/rerun/compare/v0.3.1...v0.4.0) - Outlines, web viewer and performance improvements - 2023-03-28

https://user-images.githubusercontent.com/1220815/228241887-03b311e2-80e9-4541-9281-6d334a15ab04.mp4

### Overview & Highlights
* Add support for mesh vertex colors [#1671](https://github.com/rerun-io/rerun/pull/1671)
* Lower memory use [#1535](https://github.com/rerun-io/rerun/pull/1535)
* Improve garbage collection [#1560](https://github.com/rerun-io/rerun/pull/1560)
* Improve the web viewer [#1596](https://github.com/rerun-io/rerun/pull/1596) [#1594](https://github.com/rerun-io/rerun/pull/1594) [#1682](https://github.com/rerun-io/rerun/pull/1682) [#1716](https://github.com/rerun-io/rerun/pull/1716) …
* Nice outlines when hovering/selecting
* Add an example of forever-streaming a web-camera image to Rerun [#1502](https://github.com/rerun-io/rerun/pull/1502)
* Fix crash-on-save on some versions of Linux [#1402](https://github.com/rerun-io/rerun/pull/1402)
* And a lot of other bug fixes
* Many performance improvements

We now host an experimental and unpolished web-viewer at <https://app.rerun.io/> for anyone to try out!

### In Detail

#### 🐍 Python SDK
- Expose all Rerun enums and types to main module scope [#1598](https://github.com/rerun-io/rerun/pull/1598)
- Make `log_point` more forgiving and update docstring [#1663](https://github.com/rerun-io/rerun/pull/1663)
- Add support for mesh vertex colors [#1671](https://github.com/rerun-io/rerun/pull/1671)

#### 🦀 Rust SDK
- ⚠️ `Session::new` has been replaced with `SessionBuilder` [#1528](https://github.com/rerun-io/rerun/pull/1528)
- ⚠️ `session.spawn(…)` -> `rerun::native_viewer::spawn(session, …)` [#1507](https://github.com/rerun-io/rerun/pull/1507)
- ⚠️ `session.show()` -> `rerun::native_viewer::show(session)` [#1507](https://github.com/rerun-io/rerun/pull/1507)
- ⚠️ `session.serve(…)` -> `rerun::serve_web_viewer(session, …);` [#1507](https://github.com/rerun-io/rerun/pull/1507)
- ⚠️ `rerun::global_session` is now hidden behind the `global_session` feature flag [#1507](https://github.com/rerun-io/rerun/pull/1507)
- Add support for mesh vertex colors [#1671](https://github.com/rerun-io/rerun/pull/1671)

#### 🪳 Bug Fixes
- datastore: disable compaction (fixes 2x memory issue) [#1535](https://github.com/rerun-io/rerun/pull/1535)
- Fix garbage collection [#1560](https://github.com/rerun-io/rerun/pull/1560)
- Avoid using undefined extern "C" on windows [#1577](https://github.com/rerun-io/rerun/pull/1577)
- Fix crash on decoding old .rrd files [#1579](https://github.com/rerun-io/rerun/pull/1579)
- datastore: stabilize dataframe sorts [#1549](https://github.com/rerun-io/rerun/pull/1549)
- Stop using infinities in wgsl shaders [#1594](https://github.com/rerun-io/rerun/pull/1594)
- Workaround for alpha to coverage state leaking on (Web)GL renderer [#1596](https://github.com/rerun-io/rerun/pull/1596)
- Use a patched `wasm-bindgen-cli` with fix for 2GiB bug [#1605](https://github.com/rerun-io/rerun/pull/1605)
- Misc: make example in `log_pinhole` runnable [#1609](https://github.com/rerun-io/rerun/pull/1609) (thanks [@Sjouks](https://github.com/Sjouks)!)
- Early-out on zero-sized space-views to prevent crashes [#1623](https://github.com/rerun-io/rerun/pull/1623)
- Print our own callstack on panics [#1622](https://github.com/rerun-io/rerun/pull/1622)
- Handle ctrl+c to gracefully shutdown the server(s) [#1613](https://github.com/rerun-io/rerun/pull/1613)
- Fix crash on serve exit, second attempt [#1633](https://github.com/rerun-io/rerun/pull/1633)
- Fix wrong remove-tooltip for entities and groups [#1637](https://github.com/rerun-io/rerun/pull/1637)
- Fix requiring focus for shutdown via ctrl+c when starting viewer from command line [#1646](https://github.com/rerun-io/rerun/pull/1646)
- Fix eye spin after eye reset [#1652](https://github.com/rerun-io/rerun/pull/1652)
- Fix crash on negative radii by instead warning [#1654](https://github.com/rerun-io/rerun/pull/1654)
- Fix crash when trying to listen on a taken TCP port [#1650](https://github.com/rerun-io/rerun/pull/1650)
- Don't show 2D labels in 3D space views. [#1641](https://github.com/rerun-io/rerun/pull/1641)
- Fix Z fighting with improved depth offset math [#1661](https://github.com/rerun-io/rerun/pull/1661)
- Whether a spatial view is 2d or 3d is now reevaluated over time unless picked explicitly [#1660](https://github.com/rerun-io/rerun/pull/1660)
- Update wgpu to v0.15.3, fixing meshes on Windows Chrome [#1682](https://github.com/rerun-io/rerun/pull/1682)
- Fix a bug in the image hover code, causing the wrong RGBA values to be printed [#1690](https://github.com/rerun-io/rerun/pull/1690)
- Fix a bug that caused points to be render too large [#1690](https://github.com/rerun-io/rerun/pull/1690)
- Fix web crash on missing uniform buffer padding [#1699](https://github.com/rerun-io/rerun/pull/1699)
- Fix `memory_usage` example relying on implicit recursive features [#1709](https://github.com/rerun-io/rerun/pull/1709)
- Track changed state in nav mode combo box [#1703](https://github.com/rerun-io/rerun/pull/1703)
- Fix crash-on-save by switching file-picker dialog to `xdg-portal` [#1402](https://github.com/rerun-io/rerun/pull/1402)
- Change roll-shortcut from ALT to SHIFT [#1715](https://github.com/rerun-io/rerun/pull/1715)
- Fix CpuWriteGpuReadBelt producing unaligned gpu buffer offsets [#1716](https://github.com/rerun-io/rerun/pull/1716)
- Fix arrows requiring a radius to be visible [#1720](https://github.com/rerun-io/rerun/pull/1720)

#### 🚀 Performance Improvements
- Add re_arrow_store profile scopes [#1546](https://github.com/rerun-io/rerun/pull/1546)
- datastore: early exit missing components at table level [#1554](https://github.com/rerun-io/rerun/pull/1554)
- datastore: track bucket count in store stats & mem panel [#1555](https://github.com/rerun-io/rerun/pull/1555)
- LogDb: don't split on index bucket size [#1558](https://github.com/rerun-io/rerun/pull/1558)
- Introduce a simpler cache dedicated to just decode JPEGs [#1550](https://github.com/rerun-io/rerun/pull/1550)
- Implement outlines for points 2d/3d/depth & use them for select & hover in Viewer [#1568](https://github.com/rerun-io/rerun/pull/1568)
- Simplify ImageCache [#1551](https://github.com/rerun-io/rerun/pull/1551)
- New time panel density graph [#1557](https://github.com/rerun-io/rerun/pull/1557)
- Refactor the Arrow Mesh3D type to use zero-copy Buffers [#1691](https://github.com/rerun-io/rerun/pull/1691)
- Remove the redundant costly transform check during categorization [#1695](https://github.com/rerun-io/rerun/pull/1695)
- batching 3: `DataRow` & `DataTable` + no bundles outside of transport [#1673](https://github.com/rerun-io/rerun/pull/1673)

#### 🧑‍🏫 Examples
- Very simple example streaming from an opencv camera [#1502](https://github.com/rerun-io/rerun/pull/1502)
- Initial TurtleBot subscriber demo [#1523](https://github.com/rerun-io/rerun/pull/1523)

#### 📚 Docs
- Link to the Python SDK build instructions in `rerun_py/README.md` [#1565](https://github.com/rerun-io/rerun/pull/1565)

#### 🖼 UI Improvements
- Fix combining outline mask for selection & hover [#1552](https://github.com/rerun-io/rerun/pull/1552)
- Implement outlines for rectangles & use them for select & hover of image primitives in Viewer [#1559](https://github.com/rerun-io/rerun/pull/1559)
- Show log messages in egui toast notifications [#1603](https://github.com/rerun-io/rerun/pull/1603)
- Adapt UI for smaller screens [#1608](https://github.com/rerun-io/rerun/pull/1608)
- Nicer toast notifications [#1621](https://github.com/rerun-io/rerun/pull/1621)
- Don't hover things in 2D/3D views if we are dragging something [#1643](https://github.com/rerun-io/rerun/pull/1643)
- Allow rolling 3D camera with primary mouse button + alt modifier [#1659](https://github.com/rerun-io/rerun/pull/1659)
- Name space views after the space and indicate duplicate names [#1653](https://github.com/rerun-io/rerun/pull/1653)
- Add banner about mobile browsers being unsupported [#1674](https://github.com/rerun-io/rerun/pull/1674)
- Improve ui for tensors and color map selection [#1683](https://github.com/rerun-io/rerun/pull/1683)
- Only show the mobile OS warning banner on web [#1685](https://github.com/rerun-io/rerun/pull/1685)
- Improve the depth backprojection feature [#1690](https://github.com/rerun-io/rerun/pull/1690)
- Swap overlay order of selection & hover outlines [#1705](https://github.com/rerun-io/rerun/pull/1705)
- Turn on depth cloud backprojection by default [#1710](https://github.com/rerun-io/rerun/pull/1710)
- Add radius boost for depth clouds on outline [#1713](https://github.com/rerun-io/rerun/pull/1713)

#### 🤷‍♂️ Other Viewer Improvements
- Fix web feature name in error messages [#1521](https://github.com/rerun-io/rerun/pull/1521)
- Use outlines for mesh selections instead of highlight colors [#1540](https://github.com/rerun-io/rerun/pull/1540)
- Implement outlines for line renderer & use them for select & hover of "line-like" primitives in Viewer [#1553](https://github.com/rerun-io/rerun/pull/1553)
- Load .rrd file over HTTP [#1600](https://github.com/rerun-io/rerun/pull/1600)
- Revert "Handle ctrl+c to gracefully shutdown the server(s)" [#1632](https://github.com/rerun-io/rerun/pull/1632)
- More eager GC, and remove `--fast-math` optimization for wasm [#1656](https://github.com/rerun-io/rerun/pull/1656)
- Detect failure to install GUI log callback [#1655](https://github.com/rerun-io/rerun/pull/1655)
- Warn when most of the RAM has been used up by Rerun [#1651](https://github.com/rerun-io/rerun/pull/1651)
- Apply color maps to all types of depth tensors [#1686](https://github.com/rerun-io/rerun/pull/1686)
- Size boosted outlines for points & lines, color & size tweaking [#1667](https://github.com/rerun-io/rerun/pull/1667)
- Default point radius to 1.5 ui points [#1706](https://github.com/rerun-io/rerun/pull/1706)
- When streaming an rrd from http: play it, don't follow it [#1707](https://github.com/rerun-io/rerun/pull/1707)

#### 🕸️ Web
- Use `log` as our log backend instead of `tracing` [#1590](https://github.com/rerun-io/rerun/pull/1590)
- Turn on allocation tracker at run-time and for web [#1591](https://github.com/rerun-io/rerun/pull/1591)
- Set correct MIME types in re_web_viewer_server [#1602](https://github.com/rerun-io/rerun/pull/1602)
- Upload web viewer to a bucket [#1606](https://github.com/rerun-io/rerun/pull/1606)
- Use hostname for default websocket address [#1664](https://github.com/rerun-io/rerun/pull/1664)
- Upload the colmap rrd file to gcloud [#1666](https://github.com/rerun-io/rerun/pull/1666)
- Show a warning by default on mobile browsers [#1670](https://github.com/rerun-io/rerun/pull/1670)
- Add analytics to the hosted index.html [#1675](https://github.com/rerun-io/rerun/pull/1675)
- Always upload latest prerelease to a dedicated prefix [#1676](https://github.com/rerun-io/rerun/pull/1676)
- Allow url param override on app.rerun.io [#1678](https://github.com/rerun-io/rerun/pull/1678)
- Show the git commit in the about section in pre-release builds [#1677](https://github.com/rerun-io/rerun/pull/1677)
- Update the web icon [#1688](https://github.com/rerun-io/rerun/pull/1688)

#### 🎨 Renderer Improvements
- Outlines via masking & postprocessing in `re_renderer` [#1532](https://github.com/rerun-io/rerun/pull/1532)
- Add missing profiling scopes in `re_renderer` [#1567](https://github.com/rerun-io/rerun/pull/1567)
- Don't call `wgpu::Device::poll` on the web [#1626](https://github.com/rerun-io/rerun/pull/1626)
- Merge final outline render into composite step in order to fix blending [#1629](https://github.com/rerun-io/rerun/pull/1629)
- renderer: fix the groupby logic in mesh instancing [#1657](https://github.com/rerun-io/rerun/pull/1657)
- Fix outlines being offset diagonally by about half a pixel [#1668](https://github.com/rerun-io/rerun/pull/1668)
- Gpu readback belt for fast & easy data readback from gpu [#1687](https://github.com/rerun-io/rerun/pull/1687)
- Make CpuWriteGpuReadBelt texture copies easier/less error prone [#1689](https://github.com/rerun-io/rerun/pull/1689)

#### ✨ Other Enhancement
- datastore: split out formatting & sanity checks in their own modules [#1625](https://github.com/rerun-io/rerun/pull/1625)
- Add `rerun --save`: stream incoming log stream to an rrd file [#1662](https://github.com/rerun-io/rerun/pull/1662)
- batching 1: introduce `DataCell` & retire `ComponentBundle` [#1634](https://github.com/rerun-io/rerun/pull/1634)
- Data store batching 2: split out component traits [#1636](https://github.com/rerun-io/rerun/pull/1636)

#### 📈 Analytics
- Analytics: don't spam warning when there is an HTTP connection problem [#1564](https://github.com/rerun-io/rerun/pull/1564)
- Analytics: Rename "location" to "file_line" in the "crash-panic" event [#1575](https://github.com/rerun-io/rerun/pull/1575)

#### 🗣 Merged RFCs
- RFC: component-datatype conversions [#1595](https://github.com/rerun-io/rerun/pull/1595)
- RFC: pre-proposal for blueprint store [#1582](https://github.com/rerun-io/rerun/pull/1582)

#### 🧑‍💻 Dev-experience
- Update `rayon` [#1541](https://github.com/rerun-io/rerun/pull/1541)
- Fix some `1.68` clippy lints [#1569](https://github.com/rerun-io/rerun/pull/1569)
- Remove duplicated 'nix' crate [#1479](https://github.com/rerun-io/rerun/pull/1479)
- Better MsgId format [#1566](https://github.com/rerun-io/rerun/pull/1566)
- Lint vertical spacing in Rust code [#1572](https://github.com/rerun-io/rerun/pull/1572)
- CI: Replace wasm_bindgen_check.sh with actually building the web-viewer [#1604](https://github.com/rerun-io/rerun/pull/1604)
- Add --all-features to Rust Analyzer flags [#1624](https://github.com/rerun-io/rerun/pull/1624)
- Run clippy for wasm, with own clippy.toml config file [#1628](https://github.com/rerun-io/rerun/pull/1628)
- Update tokio v1.24.1 -> v1.26.0 [#1635](https://github.com/rerun-io/rerun/pull/1635)
- Add a workflow input for running benchmarks manually [#1698](https://github.com/rerun-io/rerun/pull/1698)
- Add missing } to fix rust workflow [#1700](https://github.com/rerun-io/rerun/pull/1700)
- Fix `lint.py` [#1719](https://github.com/rerun-io/rerun/pull/1719)
- Add a script that generates a changelog from recent PRs and their labels [#1718](https://github.com/rerun-io/rerun/pull/1718)

#### 🤷‍♂️ Other
- Clean up opencv_canny example slightly [b487e550dcb87225858dc6f76b791a25e938e75e](https://github.com/rerun-io/rerun/commit/b487e550dcb87225858dc6f76b791a25e938e75e)
- Lint fixes [9901e7c6735356b1970ddabc926bc5378d82e057](https://github.com/rerun-io/rerun/commit/9901e7c6735356b1970ddabc926bc5378d82e057)


## [0.3.1](https://github.com/rerun-io/rerun/compare/v0.3.0...v0.3.1) - Remove potentially sensitive analytics - 2023-03-13

Remove potentially sensitive analytics, including path to rerun source code on panics, and rerun branch name when building from source [#1563](https://github.com/rerun-io/rerun/pull/1563)


## [0.3.0](https://github.com/rerun-io/rerun/compare/v0.2.0...v0.3.0) - 2023-03-07
### Overview & Highlights

After a successful launch a couple of weeks ago, we're back with our second release!
With a few exceptions this release focuses on internal refactors & improving our processes.
However, we think you'll enjoy these goodies that made it in nonetheless!

https://user-images.githubusercontent.com/2910679/222510504-23871b8c-0bef-49c2-bbd2-37baab4247e8.mp4


You can now generate point clouds directly from depth textures and choose a wide variety of color maps.
Check out this [video](https://user-images.githubusercontent.com/1220815/223365363-da13585f-3a91-4cb8-a6ef-8a6fadbeb4eb.webm) on how to use it.
This is **a lot** faster and more convenient than doing so manually in your own code
Some caveats: Picking is not yet working and visible history may behave differently (related to [#723](https://github.com/rerun-io/rerun/issues/723))

Other highlights:

* Viewer
  * Improved formatting of date-times in plots [#1356](https://github.com/rerun-io/rerun/pull/1356)
  * Labels for 3D objects have now a color can now be selected & hovered [#1438](https://github.com/rerun-io/rerun/pull/1438)
  * Scale factor is saved across sessions and more persistent between screens [#1448](https://github.com/rerun-io/rerun/pull/1448)
  * Showing tensors in the viewer is now faster
* SDK
  * Python packages now work with Ubuntu-20.04 [#1334](https://github.com/rerun-io/rerun/pull/1334)
  * u8 segmentation stay u8 now (they converted to u16 before) [#1376](https://github.com/rerun-io/rerun/pull/1376)
  * 2D Line strips can now be logged directly [#1430](https://github.com/rerun-io/rerun/pull/1430)
  * Add a `strict` mode to the Python SDK where misuses of the API result in exceptions being raised.[#1477](https://github.com/rerun-io/rerun/pull/1477)
  * Fix disabling Python API through `init` not working [#1517](https://github.com/rerun-io/rerun/pull/1517)
* General
  * We build now with fewer build dependencies (there is however [still more work to do!](https://github.com/rerun-io/rerun/issues/1316)).
  Notably, we previously used a version of the `time` crate which had a security issue (CVE-2020-26235), thanks @mpizenberg for helping out!
  * Print more information & troubleshooting info on crash

Meanwhile, we did a bunch of improvements to our manual. If you had trouble running Rerun so far, check our updated [troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting) page (and as always, please [open an issue](https://github.com/rerun-io/rerun/issues/new/choose) if something doesn't work).

⚠️ BREAKING: old `.rrd` files no longer load ⚠️

### In Detail
#### New Features
* Generate point clouds directly from depth textures
  * re_renderer: implement depth cloud renderer [#1415](https://github.com/rerun-io/rerun/pull/1415)
  * Integrate depth clouds into Rerun [#1421](https://github.com/rerun-io/rerun/pull/1421)
  * CPU & GPU color maps [#1484](https://github.com/rerun-io/rerun/pull/1484)
  * Integrate GPU color maps into depth clouds [#1486](https://github.com/rerun-io/rerun/pull/1486)
* Python SDK: Add strict mode [#1477](https://github.com/rerun-io/rerun/pull/1477)
* OS independent Zoom factor & serialization thereof [#1448](https://github.com/rerun-io/rerun/pull/1448)
* Labels for 3D objects have now a color can now be selected & hovered [#1438](https://github.com/rerun-io/rerun/pull/1438)
* Add 2d support for linestrips [#1430](https://github.com/rerun-io/rerun/pull/1430)
* Add signal handler on *nix with troubleshooting and stacktrace [#1340](https://github.com/rerun-io/rerun/pull/1340)
  * Point users to our troubleshooting page on panic [#1338](https://github.com/rerun-io/rerun/pull/1338)

#### Performance
* Speed up conversions for color arrays in Python [#1454](https://github.com/rerun-io/rerun/pull/1454)
* Speed up fixed-sized array iteration [#1050](https://github.com/rerun-io/rerun/pull/1050)
* Speed up tensor handling by padding data through more directly
  * Direct conversion to dynamic image from Tensors [#1455](https://github.com/rerun-io/rerun/pull/1455)
  * Convert view_tensor to use the new native Tensors [#1439](https://github.com/rerun-io/rerun/pull/1439)
* Add option to show performance metrics in the UI in release builds too [#1444](https://github.com/rerun-io/rerun/pull/1444)
* Faster stable diffusion sample [#1364](https://github.com/rerun-io/rerun/pull/1364)
* SDK: stream to disk with `save` feature [#1405](https://github.com/rerun-io/rerun/pull/1405)
* `re_renderer` has now a direct CPU->GPU copy mechanism
  * `CpuWriteGpuReadBelt` for fast frame by frame memory transfers [#1382](https://github.com/rerun-io/rerun/pull/1382)
  * Uniform buffer utility using `CpuWriteGpuReadBelt` [#1400](https://github.com/rerun-io/rerun/pull/1400)
  * Use `CpuWriteGpuReadBelt` for mesh data gpu upload [#1416](https://github.com/rerun-io/rerun/pull/1416)

#### Small improvements & Bugfixes
* UI
  * Add scroll-bars the "Add/Remove entities" window [#1445](https://github.com/rerun-io/rerun/pull/1445)
  * Unify the time formatting between the time panel and the plot [#1369](https://github.com/rerun-io/rerun/pull/1369)
  * Timeline
    * Fix precision issue when zooming in on the timeline [#1370](https://github.com/rerun-io/rerun/pull/1370)
    * Improve the gap-detector [#1363](https://github.com/rerun-io/rerun/pull/1363)
  * Better time axis on plot view [#1356](https://github.com/rerun-io/rerun/pull/1356)
  * Prevent wrap on 'Streams' text [#1308](https://github.com/rerun-io/rerun/pull/1308)
  * Update to eframe 0.21.3 with fix for web text input [#1311](https://github.com/rerun-io/rerun/pull/1311)
* `re_renderer`
  * Fix crash due to always expecting Rgba8Unorm backbuffer on Web & Bgra8Unorm on native [#1413](https://github.com/rerun-io/rerun/pull/1413)
  * Allow controlling the graphics backend & power preference through standard wgpu env vars [#1332](https://github.com/rerun-io/rerun/pull/1332)
* Heuristic for camera frustum length is now based on scene size [#1433](https://github.com/rerun-io/rerun/pull/1433)
* Fix python type signature for tensor names [#1443](https://github.com/rerun-io/rerun/pull/1443)
* Don't convert u8 segmentation images to u16 [#1376](https://github.com/rerun-io/rerun/pull/1376)
* Docs (excluding the manual)
  * Improve the docs of `connect` and `serve` [#1450](https://github.com/rerun-io/rerun/pull/1450)
  * Update log_mesh and log_meshes docs. [#1286](https://github.com/rerun-io/rerun/pull/1286)
  * Add guidelines for adding dependencies in a PR [#1431](https://github.com/rerun-io/rerun/pull/1431)
  * Add a few more sections to `CODE_STYLE.md` [#1365](https://github.com/rerun-io/rerun/pull/1365)
  * Fixup for some doc links [#1314](https://github.com/rerun-io/rerun/pull/1314)
  * Document undocumented environment variables on help page. [#1335](https://github.com/rerun-io/rerun/pull/1335)
  * Link to SDK operating modes doc in both SDK [#1330](https://github.com/rerun-io/rerun/pull/1330)
* More information in `--version` [#1388](https://github.com/rerun-io/rerun/pull/1388)
* Remove already broken `show` method from Python SDK [#1429](https://github.com/rerun-io/rerun/pull/1429)
* Analytics
  * Send analytics events with callstacks on panics and signals [#1409](https://github.com/rerun-io/rerun/pull/1409)
  * Put all analytics to one bucket [#1390](https://github.com/rerun-io/rerun/pull/1390)
  * add event for when we serve the web-viewer .wasm [#1379](https://github.com/rerun-io/rerun/pull/1379)
  * register SDK language and data source [#1371](https://github.com/rerun-io/rerun/pull/1371)
  * Refactor analytics [#1368](https://github.com/rerun-io/rerun/pull/1368)
* Versioned log streams [#1420](https://github.com/rerun-io/rerun/pull/1420)
* Fix path issues when running debug viewer within workspace [#1341](https://github.com/rerun-io/rerun/pull/1341)
* Detailed errors for re_renderer `include_file!` [#1339](https://github.com/rerun-io/rerun/pull/1339)
* Limit logging in web-viewer to `warn` in order to workaround a crash issue (and reduce log spam) [1514](https://github.com/rerun-io/rerun/pull/1514)
* Fix disabling API through `init` not working [#1517](https://github.com/rerun-io/rerun/pull/1517)

#### CI, Testing & Build improvements
* Reduce build dependencies
  * Get rid of time 0.1.* dependency [#1408](https://github.com/rerun-io/rerun/pull/1408)
  * Remove unnecessary ordered-float [#1461](https://github.com/rerun-io/rerun/pull/1461)
  * Remove extraneous `image` features and dependencies [#1425](https://github.com/rerun-io/rerun/pull/1425)
  * Replace `reqwest` with `ureq` [#1407](https://github.com/rerun-io/rerun/pull/1407)
  * Remove derive_more dependency [#1406](https://github.com/rerun-io/rerun/pull/1406)
* Use different artifact names for wasm/js in debug builds [#1428](https://github.com/rerun-io/rerun/pull/1428)
* Separate mac wheels & trigger wheel build from ui [#1499](https://github.com/rerun-io/rerun/pull/1499)
* Add spell checking to CI [#1492](https://github.com/rerun-io/rerun/pull/1492)
* Repo size
  * Always create new orphaned branch for gh-pages [#1490](https://github.com/rerun-io/rerun/pull/1490)
  * GitHub Action to prevent large files [#1478](https://github.com/rerun-io/rerun/pull/1478)
* Python
  * Remove the python job path filters [#1452](https://github.com/rerun-io/rerun/pull/1452)
  * Use ruff for our python lints [#1378](https://github.com/rerun-io/rerun/pull/1378)
  * Use python3 in the jobs that weren't tested in PR [#1348](https://github.com/rerun-io/rerun/pull/1348)
* Testing
  * Add a test of memory use when logging a lot of big images [#1372](https://github.com/rerun-io/rerun/pull/1372)
* Switch ci_docker to a container based on ubuntu 20.04 [#1334](https://github.com/rerun-io/rerun/pull/1334)
* Release handling
  * Switch release action to ncipollo [#1489](https://github.com/rerun-io/rerun/pull/1489)
  * Fix our continuous pre-releases [#1458](https://github.com/rerun-io/rerun/pull/1458)
  * Delete the prerelease before creating the new one [#1485](https://github.com/rerun-io/rerun/pull/1485)
  * Set prerelease to true even for version-tagged CI job [#1504](https://github.com/rerun-io/rerun/pull/1504)
  * Let the release job take care of creating the tag [#1501](https://github.com/rerun-io/rerun/pull/1501)
  * Use `cargo update -w` instead of `cargo check` when prepping prerelease [#1500](https://github.com/rerun-io/rerun/pull/1500)
  * Use prerelease tag instead of latest and update pointer on prerelease [#1481](https://github.com/rerun-io/rerun/pull/1481)
  * Include date in pre-release version [#1472](https://github.com/rerun-io/rerun/pull/1472)
  * Switch pre-release action to ncipollo/release-action [#1466](https://github.com/rerun-io/rerun/pull/1466)
* Disallow some methods and types via Clippy[#1411](https://github.com/rerun-io/rerun/pull/1411)

#### Other non-user-facing refactors
* Fix: don't create a dummy LogDb when opening the Rerun Menu [#1440](https://github.com/rerun-io/rerun/pull/1440)
* `re_renderer`
  * `Draw Phases` in preparation of executing `Renderer` several times on different targets [#1419](https://github.com/rerun-io/rerun/pull/1419)
    * Fix mesh creation failing to copy index data. [#1473](https://github.com/rerun-io/rerun/pull/1473)
    * do not silently drop draw phases [#1471](https://github.com/rerun-io/rerun/pull/1471)
  * Simplify bind group allocation call by passing pool collection object. [#1459](https://github.com/rerun-io/rerun/pull/1459)
  * Interior mutable buffer/texture/bindgroup pools [#1374](https://github.com/rerun-io/rerun/pull/1374)
  * Rename all instances of `frame_maintenance` to `begin_frame` [#1360](https://github.com/rerun-io/rerun/pull/1360)
  * Texture & buffer call now wgpu's `destroy` on removal from pool [#1359](https://github.com/rerun-io/rerun/pull/1359)
  * Arrow buffers as (optional) first-class citizen [#1482](https://github.com/rerun-io/rerun/pull/1482)
  * Log static re_renderer resource generation [#1464](https://github.com/rerun-io/rerun/pull/1464)
* Internal log_text_entry_internal to break circular deps [#1488](https://github.com/rerun-io/rerun/pull/1488)
* Delete ClassicTensor and cleanup [#1456](https://github.com/rerun-io/rerun/pull/1456)
* Fix re_renderer file watcher watching the same file several times [#1463](https://github.com/rerun-io/rerun/pull/1463)
* Analytics
  * More ergonomic API [#1410](https://github.com/rerun-io/rerun/pull/1410)
  * Streamlining host vs. recorder python/rust versions [#1380](https://github.com/rerun-io/rerun/pull/1380)
  * Fix workspace detection [#1437](https://github.com/rerun-io/rerun/pull/1437)
* Introduce `DeserializableComponent` trait and high-level `query_latest` [#1417](https://github.com/rerun-io/rerun/pull/1417)


[Full Changelog](https://github.com/rerun-io/rerun/compare/v0.2.0...v0.3.0)

## 0.2.0 - 2023-02-14
First public release!
