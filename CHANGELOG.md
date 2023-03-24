# Rerun changelog


## Unreleased
[Commits since latest release](https://github.com/rerun-io/rerun/compare/latest...HEAD)

* Fixed a bug in the image hover code, causing the wrong RGBA values to be printed 😬 [#1690](https://github.com/rerun-io/rerun/pull/1356)
* Fixed a bug that caused points to be render too large [#1690](https://github.com/rerun-io/rerun/pull/1690)

## 0.3.1 - Remove potentially sensitive analytics
[Commits](https://github.com/rerun-io/rerun/compare/v0.3.1...v0.3.0)

Remove potentially sensitive analytics, including path to rerun source code on panics, and rerun branch name when building from source [#1563](https://github.com/rerun-io/rerun/pull/1563)


## 0.3.0
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

#### Other not user facing refactors
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
