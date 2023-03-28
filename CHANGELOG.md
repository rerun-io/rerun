# Rerun changelog


## [Unreleased](https://github.com/rerun-io/rerun/compare/latest...HEAD)
…

## [0.4.0](https://github.com/rerun-io/rerun/compare/v0.3.1...v0.4.0) - Outlines, web viewer and performance improvements

https://user-images.githubusercontent.com/1220815/228241887-03b311e2-80e9-4541-9281-6d334a15ab04.mp4


## Overview & Highlights
* Add support for mesh vertex colors [#1671](https://github.com/rerun-io/rerun/pull/1671)
* Lower memory use [#1535](https://github.com/rerun-io/rerun/pull/1535)
* Improve garbage collection [#1560](https://github.com/rerun-io/rerun/pull/1560)
* Improve the web viewer [#1596](https://github.com/rerun-io/rerun/pull/1596) [#1594](https://github.com/rerun-io/rerun/pull/1594) [#1682](https://github.com/rerun-io/rerun/pull/1682)  [#1716](https://github.com/rerun-io/rerun/pull/1716) …
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
- ⚠️ `session.show()` -> `rerun::native_viewer::show(session)`  [#1507](https://github.com/rerun-io/rerun/pull/1507)
- ⚠️ `session.serve(…)` -> `rerun::serve_web_viewer(session, …);`  [#1507](https://github.com/rerun-io/rerun/pull/1507)
- ⚠️ `rerun::global_session` is now hidden behind the `global_session` feature flag  [#1507](https://github.com/rerun-io/rerun/pull/1507)
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
- Misc: make example in `log_pinhole` runable [#1609](https://github.com/rerun-io/rerun/pull/1609) (thanks [@Sjouks](https://github.com/Sjouks)!)
- Early-out on zero-sized space-views to prevent crashes [#1623](https://github.com/rerun-io/rerun/pull/1623)
- Print our own callstack on panics [#1622](https://github.com/rerun-io/rerun/pull/1622)
- Handle ctrl+c to gracefully shutdown the server(s) [#1613](https://github.com/rerun-io/rerun/pull/1613)
- Fix crash on serve exit, second attempt [#1633](https://github.com/rerun-io/rerun/pull/1633)
- Fix wrong remove-tooltip for entities and groups [#1637](https://github.com/rerun-io/rerun/pull/1637)
- Fix requiring requiring focus for shutdown via ctrl+c when starting viewer from command line [#1646](https://github.com/rerun-io/rerun/pull/1646)
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
- LogDb: dont split on index bucket size [#1558](https://github.com/rerun-io/rerun/pull/1558)
- Introduce a simpler cache dedicated to just decode JPEGs  [#1550](https://github.com/rerun-io/rerun/pull/1550)
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


## [0.3.1](https://github.com/rerun-io/rerun/compare/v0.3.0...v0.3.1) - Remove potentially sensitive analytics

Remove potentially sensitive analytics, including path to rerun source code on panics, and rerun branch name when building from source [#1563](https://github.com/rerun-io/rerun/pull/1563)


## [0.3.0](https://github.com/rerun-io/rerun/compare/v0.2.0...v0.3.0)
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
  * Integrate GPU color maps into depth clouds  [#1486](https://github.com/rerun-io/rerun/pull/1486)
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
* Versioned log streams streams [#1420](https://github.com/rerun-io/rerun/pull/1420)
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

## 0.2.0
First public release!
