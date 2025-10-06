
TODO: add link to release video

📖 Release blogpost: TODO: add link

🧳 Migration guide: TODO: add link

### ✨ Overview & highlights
TODO: fill in

### ⚠️ Breaking changes
TODO: fill in
🧳 Migration guide: TODO: add link (yes, again)

### 🔎 Details

#### 🐍 Python API
- Add module definition to all `pyclasses` [#11268](https://github.com/rerun-io/rerun/pull/11268)
- Python SDK: Add `timeout_sec` argument to `flush` [#11295](https://github.com/rerun-io/rerun/pull/11295)
- Python SDK: remove `blocking` argument of `flush` [#11314](https://github.com/rerun-io/rerun/pull/11314)
- Fix instances of `newest_first` not working correctly [#11326](https://github.com/rerun-io/rerun/pull/11326)
- Update Schema to make query view requirements clearer [#11287](https://github.com/rerun-io/rerun/pull/11287)
- Fix error when logging `AnyValues` with empty columns [#11322](https://github.com/rerun-io/rerun/pull/11322)
- Include "outer nullability" when we format column datatypes [#11339](https://github.com/rerun-io/rerun/pull/11339)
- Fix error message for what package to install [#11398](https://github.com/rerun-io/rerun/pull/11398)
- Py-SDK: More kw-args [#11384](https://github.com/rerun-io/rerun/pull/11384)
- Add meaningful equality comparisons to many rust wrappers [#11401](https://github.com/rerun-io/rerun/pull/11401)

#### 🪳 Bug fixes
- Fix edge case for parsing videos with constant frame size [#11226](https://github.com/rerun-io/rerun/pull/11226)
- Fix title bar height on macOS Tahoe [#11241](https://github.com/rerun-io/rerun/pull/11241)
- Blueprint panel can now always be resized [#11046](https://github.com/rerun-io/rerun/pull/11046)
- Fix rare issue where video stream sample indices would be determined incorrectly, breaking video inspection UI & playback [#11308](https://github.com/rerun-io/rerun/pull/11308)
- Fix links to custom timelines [#11333](https://github.com/rerun-io/rerun/pull/11333)
- Fix url edit warning spam [#11330](https://github.com/rerun-io/rerun/pull/11330)
- Fix race condition for ui adjustments while loading recordings from redap client [#11365](https://github.com/rerun-io/rerun/pull/11365)
- Fix formatting and parsing of component paths in URLs [#11364](https://github.com/rerun-io/rerun/pull/11364)
- Share button on notebook no longer uses the current base url for web viewer urls [#11379](https://github.com/rerun-io/rerun/pull/11379)
- Enable "Close current recording" only when there's a recording [#11353](https://github.com/rerun-io/rerun/pull/11353)

#### 🌁 Viewer improvements
- Keep last paused time in web-viewer url [#11246](https://github.com/rerun-io/rerun/pull/11246)
- Add limited support for out-of-order video stream samples [#11307](https://github.com/rerun-io/rerun/pull/11307)
- Better video stream errors for missing samples & key frames [#11310](https://github.com/rerun-io/rerun/pull/11310)
- Add optional viewer url parameter to web app options [#11296](https://github.com/rerun-io/rerun/pull/11296)
- Add spectral colormap [#11298](https://github.com/rerun-io/rerun/pull/11298)
- Better gRPC errors [#11335](https://github.com/rerun-io/rerun/pull/11335)

#### 🗄️ OSS server
- Add table support to OSS server [#11356](https://github.com/rerun-io/rerun/pull/11356)

#### 📚 Docs
- Add how to connect the server to the CLI command output [#11400](https://github.com/rerun-io/rerun/pull/11400)

#### 🖼 UI improvements
- Add copy-button to the recording link table item [#11242](https://github.com/rerun-io/rerun/pull/11242)
- Add copy link context menu to server entries [#11235](https://github.com/rerun-io/rerun/pull/11235)
- Add support for displaying timezone with our timestamps [#11234](https://github.com/rerun-io/rerun/pull/11234)
- Show loading screen when opening link [#11270](https://github.com/rerun-io/rerun/pull/11270)
- Support filtering by timestamp in table view [#11227](https://github.com/rerun-io/rerun/pull/11227)
- Add `starts with`/`ends with` string filters to table [#11341](https://github.com/rerun-io/rerun/pull/11341)
- Mark valid data ranges in timeline when loading data via range-limited URL [#11340](https://github.com/rerun-io/rerun/pull/11340)
- Add `does not contain` operator to string column filtering [#11349](https://github.com/rerun-io/rerun/pull/11349)
- Copy button on some selection items [#11337](https://github.com/rerun-io/rerun/pull/11337)
- Add `is not` to timestamp filtering [#11366](https://github.com/rerun-io/rerun/pull/11366)
- Add `is not` to nullable boolean filter [#11371](https://github.com/rerun-io/rerun/pull/11371)
- Treat`!=` filtering of numerical column as the inverse of `==` (aka. outer-NOT and ALL semantics) [#11372](https://github.com/rerun-io/rerun/pull/11372)
- Add context menu button to copy partition name [#11378](https://github.com/rerun-io/rerun/pull/11378)
- Store (and display) recordings in insertion order [#11415](https://github.com/rerun-io/rerun/pull/11415)

#### 🎨 Renderer improvements
- Export `BindGroupEntry` type to re_renderer rust dependents [#11406](https://github.com/rerun-io/rerun/pull/11406) (thanks [@Weijun-H](https://github.com/Weijun-H)!)

#### 🧢 MCAP
- Add support for `enum` in protobuf MCAP messages [#11280](https://github.com/rerun-io/rerun/pull/11280)

#### 🧑‍💻 Dev-experience
- Improve rrd loading errors by checking FourCC first [#11265](https://github.com/rerun-io/rerun/pull/11265)

#### 📦 Dependencies
- Update to wgpu 26 [#11300](https://github.com/rerun-io/rerun/pull/11300)
- Update DataFusion to version 49.0.2 [#11291](https://github.com/rerun-io/rerun/pull/11291)


#### Chronological changes (don't include these)
- Fix edge case for parsing videos with constant frame size [#11226](https://github.com/rerun-io/rerun/pull/11226) ce2bfd71c92a7b86ac5e53fb12929b64f44c0809
- Fix title bar height on macOS Tahoe [#11241](https://github.com/rerun-io/rerun/pull/11241) fb3590a5b506101bd5947716de4b938c33efc589
- Add copy-button to the recording link table item [#11242](https://github.com/rerun-io/rerun/pull/11242) 72ac4cbc8dbfacd2c80fb61ae8fe26a62ce8ee94
- Add copy link context menu to server entries [#11235](https://github.com/rerun-io/rerun/pull/11235) e1d3a8ba8c4221b63181457a7372387db097fe61
- Keep last paused time in web-viewer url [#11246](https://github.com/rerun-io/rerun/pull/11246) 094b8734116e9cf302b3cb9086923dca5530f3fc
- Add support for displaying timezone with our timestamps [#11234](https://github.com/rerun-io/rerun/pull/11234) 6935b54319c1b76e43a9a8e408047d0011578403
- Blueprint panel can now always be resized [#11046](https://github.com/rerun-io/rerun/pull/11046) effa0a2f23d47f100c49f11ee93c8f6832e2ba09
- Improve rrd loading errors by checking FourCC first [#11265](https://github.com/rerun-io/rerun/pull/11265) 2d0794a271549f40380655b3ec81c788426037ec
- Add support for `enum` in protobuf MCAP messages [#11280](https://github.com/rerun-io/rerun/pull/11280) 3d17d7f9096e8ffbfa39fb7c1881856f5e775134
- Update to wgpu 26 [#11300](https://github.com/rerun-io/rerun/pull/11300) 65c4dddf73a8bb36babf0f34114feb35eca0a451
- Fix rare issue where video stream sample indices would be determined incorrectly, breaking video inspection UI & playback [#11308](https://github.com/rerun-io/rerun/pull/11308) a339212ea48b24b3c9b28765fcdf6387036e115e
- Add module definition to all `pyclasses` [#11268](https://github.com/rerun-io/rerun/pull/11268) 29893f54b450b6fb40c98f68813a31ee00148235
- Add limited support for out-of-order video stream samples [#11307](https://github.com/rerun-io/rerun/pull/11307) eee28a7fa51503ad8ea13529cd1c654f879878dd
- Better video stream errors for missing samples & key frames [#11310](https://github.com/rerun-io/rerun/pull/11310) c0794d9cdf84d2a441a205b2d6e592aa9594d826
- Python SDK: Add `timeout_sec` argument to `flush` [#11295](https://github.com/rerun-io/rerun/pull/11295) 3dde2726ed56151765dfe359bd8eb547a02f1457
- Show loading screen when opening link [#11270](https://github.com/rerun-io/rerun/pull/11270) 436ea5ca618ead9a6b26171552673900fb6f4ed3
- Python SDK: remove `blocking` argument of `flush` [#11314](https://github.com/rerun-io/rerun/pull/11314) b5c3d7add12c02c3f5a6dd60dfebae8e85d10395
- Add optional viewer url parameter to web app options [#11296](https://github.com/rerun-io/rerun/pull/11296) 4aa1ff21f3e65dcdd32ee5c4e7da93d51095d8f4
- Add spectral colormap [#11298](https://github.com/rerun-io/rerun/pull/11298) 5c8022f6bdfd5018ce12bf814f0cf579c78c1fff
- Fix instances of `newest_first` not working correctly [#11326](https://github.com/rerun-io/rerun/pull/11326) fc7b7cac782e6ea6c342703da81c1b3eaf63dbf5
- Update Schema to make query view requirements clearer [#11287](https://github.com/rerun-io/rerun/pull/11287) bb3fb6fa1a70d1e6884c752336c0724bf3e6907a
- Fix error when logging `AnyValues` with empty columns [#11322](https://github.com/rerun-io/rerun/pull/11322) 0f7f7ba339693ab95de68619a30a1a69e0638d9d
- Update DataFusion to version 49.0.2 [#11291](https://github.com/rerun-io/rerun/pull/11291) 7b5245fd6d4071a9281d622639de01a9bb96f341
- Support filtering by timestamp in table view [#11227](https://github.com/rerun-io/rerun/pull/11227) 7d13956ad9dfed23f0e9e54751935434e3ddb3d4
- Fix links to custom timelines [#11333](https://github.com/rerun-io/rerun/pull/11333) 76629b919e9035e02716aa21783283bb36de2d15
- Fix url edit warning spam [#11330](https://github.com/rerun-io/rerun/pull/11330) 6808754dc17939154949d46663174511cfb64dfc
- Include "outer nullability" when we format column datatypes [#11339](https://github.com/rerun-io/rerun/pull/11339) 9869eaec8c1d8fd3328cbd9fb8ca066fb780a194
- Add `starts with`/`ends with` string filters to table [#11341](https://github.com/rerun-io/rerun/pull/11341) 5710c79827bd0c7518826abd0de41cb147f98407
- Better gRPC errors [#11335](https://github.com/rerun-io/rerun/pull/11335) bb7d135aeb8f396f2825c85e31c4399e8a86a967
- Mark valid data ranges in timeline when loading data via range-limited URL [#11340](https://github.com/rerun-io/rerun/pull/11340) 6c651ab064794c932271768dbf1cb13399a4a823
- Add `does not contain` operator to string column filtering [#11349](https://github.com/rerun-io/rerun/pull/11349) a17073721f2be045aa3befec84edc205e4f55727
- Copy button on some selection items [#11337](https://github.com/rerun-io/rerun/pull/11337) b1c64773503336f753f73d1fc33500f8c8513f29
- Add table support to OSS server [#11356](https://github.com/rerun-io/rerun/pull/11356) 21753bbae37b7593e7c2423a4c23b74419492701
- Add `is not` to timestamp filtering [#11366](https://github.com/rerun-io/rerun/pull/11366) de829816c0bf9e4bafc4ad7946551628163ee70d
- Fix race condition for ui adjustments while loading recordings from redap client [#11365](https://github.com/rerun-io/rerun/pull/11365) 61534320456bb83aef54254990fed12ef0941855
- Add `is not` to nullable boolean filter [#11371](https://github.com/rerun-io/rerun/pull/11371) 5e62eac4dbe3c5ad159143e947022312f72732dc
- Treat`!=` filtering of numerical column as the inverse of `==` (aka. outer-NOT and ALL semantics) [#11372](https://github.com/rerun-io/rerun/pull/11372) 8d338d0f531976cdff0a1e4a62c7c1af5ad68acc
- Fix formatting and parsing of component paths in URLs [#11364](https://github.com/rerun-io/rerun/pull/11364) 812a72cf370b5b1a9e1056c409cb5eb4212914ff
- Add context menu button to copy partition name [#11378](https://github.com/rerun-io/rerun/pull/11378) 76681413ab84c62e2ec389a2ca0753835d2cec40
- Share button on notebook no longer uses the current base url for web viewer urls [#11379](https://github.com/rerun-io/rerun/pull/11379) be0afb138d3b2e004c92233a3370e8a2840ff307
- Enable "Close current recording" only when there's a recording [#11353](https://github.com/rerun-io/rerun/pull/11353) 9b453bedfa3b0c9bba85dff73448abf083d36abb
- Fix error message for what package to install [#11398](https://github.com/rerun-io/rerun/pull/11398) 90479b300bd739b5e73da49b68f02fef4fcba217
- Py-SDK: More kw-args [#11384](https://github.com/rerun-io/rerun/pull/11384) 296a5744421ed1472838bea9043311e2f43d62f7
- Export `BindGroupEntry` type to re_renderer rust dependents [#11406](https://github.com/rerun-io/rerun/pull/11406) 38023e9da5194f8c2e552a8efcb546894fb4d901
- Add meaningful equality comparisons to many rust wrappers [#11401](https://github.com/rerun-io/rerun/pull/11401) 4590f7f97331acf037f4e80f9c2e1eeab47f0292
- Store (and display) recordings in insertion order [#11415](https://github.com/rerun-io/rerun/pull/11415) a9436a8efa9c16f81137aef63e464f337b5c84fb
- Add how to connect the server to the CLI command output [#11400](https://github.com/rerun-io/rerun/pull/11400) 02dabe991bd3c2cb7343ca90ba71b0165b8c1fea

