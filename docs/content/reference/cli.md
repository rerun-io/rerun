---
title: CLI manual
order: 0
---

## rerun


The Rerun command-line interface:
* Spawn viewers to visualize Rerun recordings and other supported formats.
* Start gRPC servers to share recordings over the network, on native or web.
* Inspect, edit and filter Rerun recordings.


**Usage**: ` rerun [OPTIONS] [URL_OR_PATHS]… [COMMAND]`

**Commands**

* `analytics`: Configure the behavior of our analytics.
* `rrd`: Manipulate the contents of .rrd and .rbl files.
* `reset`: Reset the memory of the Rerun Viewer.
* `man`: Generates the Rerun CLI manual (markdown).

**Arguments**

* `<URL_OR_PATHS>`
> Any combination of:
> - A gRPC url to a Rerun server
> - A path to a Rerun .rrd recording
> - A path to a Rerun .rbl blueprint
> - An HTTP(S) URL to an .rrd or .rbl file to load
> - A path to an image or mesh, or any other file that Rerun can load (see https://www.rerun.io/docs/reference/data-loaders/overview)
>
> If no arguments are given, a server will be hosted which a Rerun SDK can connect to.

**Options**

* `--bind <BIND>`
> What bind address IP to use.
>
> [Default: `0.0.0.0`]

* `--drop-at-latency <DROP_AT_LATENCY>`
> Set a maximum input latency, e.g. "200ms" or "10s".
>
> If we go over this, we start dropping packets.
>
> The default is no limit, which means Rerun might eat more and more memory and have longer and longer latency, if you are logging data faster than Rerun can index it.

* `--memory-limit <MEMORY_LIMIT>`
> An upper limit on how much memory the Rerun Viewer should use.
> When this limit is reached, Rerun will drop the oldest data.
> Example: `16GB` or `50%` (of system total).
>
> [Default: `75%`]

* `--server-memory-limit <SERVER_MEMORY_LIMIT>`
> An upper limit on how much memory the gRPC server should use.
> The server buffers log messages for the benefit of late-arriving viewers.
> When this limit is reached, Rerun will drop the oldest data.
> Example: `16GB` or `50%` (of system total).
>
> [Default: `25%`]

* `--persist-state <PERSIST_STATE>`
> Whether the Rerun Viewer should persist the state of the viewer to disk.
> When persisted, the state will be stored at the following locations:
> - Linux: `/home/UserName/.local/share/rerun`
> - macOS: `/Users/UserName/Library/Application Support/rerun`
> - Windows: `C:\Users\UserName\AppData\Roaming\rerun`
>
> [Default: `true`]

* `--port <PORT>`
> What port do we listen to for SDKs to connect to over gRPC.
>
> [Default: `9876`]

* `--profile <PROFILE>`
> Start with the puffin profiler running.
>
> [Default: `false`]

* `--save <SAVE>`
> Stream incoming log events to an .rrd file at the given path.

* `--screenshot-to <SCREENSHOT_TO>`
> Take a screenshot of the app and quit. We use this to generate screenshots of our examples. Useful together with `--window-size`.

* `--serve <SERVE>`
> Deprecated: use `--serve-web` instead.

* `--serve-web <SERVE_WEB>`
> Serve the recordings over gRPC to one or more Rerun Viewers.
>
> This will also host a web-viewer over HTTP that can connect to the gRPC address, but you can also connect with the native binary.
>
> `rerun --serve-web` will act like a proxy, listening for incoming gRPC connection from logging SDKs, and forwarding it to Rerun viewers.
>
> [Default: `false`]

* `--expect-data-soon <EXPECT_DATA_SOON>`
> This is a hint that we expect a recording to stream in very soon.
>
> This is set by the `spawn()` method in our logging SDK.
>
> The viewer will respond by fading in the welcome screen, instead of showing it directly. This ensures that it won't blink for a few frames before switching to the recording.
>
> [Default: `false`]

* `-j, --threads <THREADS>`
> The number of compute threads to use.
>
> If zero, the same number of threads as the number of cores will be used. If negative, will use that much fewer threads than cores.
>
> Rerun will still use some additional threads for I/O.
>
> [Default: `-2`]

* `--version <VERSION>`
> Print version and quit.
>
> [Default: `false`]

* `--web-viewer <WEB_VIEWER>`
> Start the viewer in the browser (instead of locally).
>
> Requires Rerun to have been compiled with the `web_viewer` feature.
>
> This implies `--serve-web`.
>
> [Default: `false`]

* `--web-viewer-port <WEB_VIEWER_PORT>`
> What port do we listen to for hosting the web viewer over HTTP. A port of 0 will pick a random port.
>
> [Default: `9090`]

* `--hide-welcome-screen <HIDE_WELCOME_SCREEN>`
> Hide the normal Rerun welcome screen.
>
> [Default: `false`]

* `--detach-process <DETACH_PROCESS>`
> Detach Rerun Viewer process from the application process.
>
> [Default: `true`]

* `--window-size <WINDOW_SIZE>`
> Set the screen resolution (in logical points), e.g. "1920x1080". Useful together with `--screenshot-to`.

* `--renderer <RENDERER>`
> Override the default graphics backend and for a specific one instead.
>
> When using `--web-viewer` this should be one of: `webgpu`, `webgl`.
>
> When starting a native viewer instead this should be one of:
>
> * `vulkan` (Linux & Windows only)
>
> * `gl` (Linux & Windows only)
>
> * `metal` (macOS only)

* `--test-receive <TEST_RECEIVE>`
> Ingest data and then quit once the goodbye message has been received.
>
> Used for testing together with `RERUN_PANIC_ON_WARN=1`.
>
> Fails if no messages are received, or if no messages are received within a dozen or so seconds.
>
> [Default: `false`]

## rerun analytics

Configure the behavior of our analytics.

**Usage**: `rerun analytics <COMMAND>`

**Commands**

* `details`: Prints extra information about analytics.
* `clear`: Deletes everything related to analytics.
* `email`: Associate an email address with the current user.
* `enable`: Enable analytics.
* `disable`: Disable analytics.
* `config`: Prints the current configuration.

## rerun analytics email

Associate an email address with the current user.

**Usage**: `rerun analytics email <EMAIL>`

**Arguments**

* `<EMAIL>`

## rerun rrd

Manipulate the contents of .rrd and .rbl files.

**Usage**: `rerun rrd <COMMAND>`

**Commands**

* `compare`: Compares the data between 2 .rrd files, returning a successful shell exit code if they match.
* `print`: Print the contents of one or more .rrd/.rbl files/streams.
* `compact`: Compacts the contents of one or more .rrd/.rbl files/streams and writes the result standard output.
* `merge`: Merges the contents of multiple .rrd/.rbl files/streams, and writes the result to standard output.
* `filter`: Filters out data from .rrd/.rbl files/streams, and writes the result to standard output.

## rerun rrd compare

Compares the data between 2 .rrd files, returning a successful shell exit code if they match.

This ignores the `log_time` timeline.

**Usage**: `rerun rrd compare [OPTIONS] <PATH_TO_RRD1> <PATH_TO_RRD2>`

**Arguments**

* `<PATH_TO_RRD1>`

* `<PATH_TO_RRD2>`

**Options**

* `--full-dump <FULL_DUMP>`
> If specified, dumps both .rrd files as tables.
>
> [Default: `false`]

## rerun rrd print

Print the contents of one or more .rrd/.rbl files/streams.

Reads from standard input if no paths are specified.

Example: `rerun rrd print /my/recordings/*.rrd`

**Usage**: `rerun rrd print [OPTIONS] [PATH_TO_INPUT_RRDS]…`

**Arguments**

* `<PATH_TO_INPUT_RRDS>`
> Paths to read from. Reads from standard input if none are specified.

**Options**

* `-v, --verbose <VERBOSE>`
> If set, print out table contents.
>
> [Default: `false`]

* `--continue-on-error <CONTINUE_ON_ERROR>`
> If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
>
> [Default: `true`]

## rerun rrd compact

Compacts the contents of one or more .rrd/.rbl files/streams and writes the result standard output.

Reads from standard input if no paths are specified.

Uses the usual environment variables to control the compaction thresholds: `RERUN_CHUNK_MAX_ROWS`, `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED`, `RERUN_CHUNK_MAX_BYTES`.

Unless explicit flags are passed, in which case they will override environment values.

Examples:

* `RERUN_CHUNK_MAX_ROWS=4096 RERUN_CHUNK_MAX_BYTES=1048576 rerun rrd compact /my/recordings/*.rrd -o output.rrd`

* `rerun rrd compact --max-rows 4096 --max-bytes=1048576 /my/recordings/*.rrd > output.rrd`

**Usage**: `rerun rrd compact [OPTIONS] [PATH_TO_INPUT_RRDS]…`

**Arguments**

* `<PATH_TO_INPUT_RRDS>`
> Paths to read from. Reads from standard input if none are specified.

**Options**

* `-o, --output <dst.(rrd|rbl)>`
> Path to write to. Writes to standard output if unspecified.

* `--max-bytes <MAX_BYTES>`
> What is the threshold, in bytes, after which a Chunk cannot be compacted any further?
>
> Overrides `RERUN_CHUNK_MAX_BYTES` if set.

* `--max-rows <MAX_ROWS>`
> What is the threshold, in rows, after which a Chunk cannot be compacted any further?
>
> Overrides `RERUN_CHUNK_MAX_ROWS` if set.

* `--max-rows-if-unsorted <MAX_ROWS_IF_UNSORTED>`
> What is the threshold, in rows, after which a Chunk cannot be compacted any further?
>
> This specifically applies to _non_ time-sorted chunks.
>
> Overrides `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` if set.

* `--continue-on-error <CONTINUE_ON_ERROR>`
> If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
>
> [Default: `false`]

## rerun rrd merge

Merges the contents of multiple .rrd/.rbl files/streams, and writes the result to standard output.

Reads from standard input if no paths are specified.

This will not affect the chunking of the data in any way.

Example: `rerun merge /my/recordings/*.rrd > output.rrd`

**Usage**: `rerun rrd merge [OPTIONS] [PATH_TO_INPUT_RRDS]…`

**Arguments**

* `<PATH_TO_INPUT_RRDS>`
> Paths to read from. Reads from standard input if none are specified.

**Options**

* `-o, --output <dst.(rrd|rbl)>`
> Path to write to. Writes to standard output if unspecified.

* `--continue-on-error <CONTINUE_ON_ERROR>`
> If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
>
> [Default: `false`]

## rerun rrd filter

Filters out data from .rrd/.rbl files/streams, and writes the result to standard output.

Reads from standard input if no paths are specified.

This will not affect the chunking of the data in any way.

Example: `rerun filter --drop-timeline log_tick /my/recordings/*.rrd > output.rrd`

**Usage**: `rerun rrd filter [OPTIONS] [PATH_TO_INPUT_RRDS]…`

**Arguments**

* `<PATH_TO_INPUT_RRDS>`
> Paths to read from. Reads from standard input if none are specified.

**Options**

* `-o, --output <dst.(rrd|rbl)>`
> Path to write to. Writes to standard output if unspecified.

* `--drop-timeline <DROPPED_TIMELINES>`
> Names of the timelines to be filtered out.

* `--drop-entity <DROPPED_ENTITY_PATHS>`
> Paths of the entities to be filtered out.

* `--continue-on-error <CONTINUE_ON_ERROR>`
> If set, will try to proceed even in the face of IO and/or decoding errors in the input data.
>
> [Default: `false`]
