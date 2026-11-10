---
title: Work with .rrd files
order: 850
description: Inspect, migrate, trim, split, merge, compact, and append to .rrd files with the rerun rrd command
---

The `rerun rrd` command can inspect and change `.rrd` files without writing any code.
See the [CLI reference](../../reference/cli.md#rerun-rrd) for all options.

Most commands write to standard output unless you pass `-o`, so you can chain them with pipes.

## Inspect a file

```sh
rerun rrd print my_recording.rrd    # what entities and components are in the file
rerun rrd stats my_recording.rrd    # sizes and chunk statistics
rerun rrd verify my_recording.rrd   # check that this version of Rerun can load the file
```

## Migrate an old file

Rerun can load files from version 0.23 and newer, and migrates them when it loads them.
To migrate a file on disk to the current version:

```sh
rerun rrd migrate my_recording.rrd
```

This writes the migrated file in place, and keeps the original as `my_recording.backup.rrd`.
Files from before 0.23 can't be migrated.

## Remove data

Drop entities or whole timelines:

```sh
rerun rrd filter --drop-entity /camera/image --drop-timeline log_tick my_recording.rrd -o smaller.rrd
```

`--drop-entity` matches the exact entity path only, not its children.
Pass it once per entity.

## Split a file

Split a recording into parts along a timeline, either at given times or into a number of equal parts:

```sh
rerun rrd split --timeline frame --time 1000 --time 2000 --output-dir ./parts my_recording.rrd
rerun rrd split --timeline frame --num-parts 4 --output-dir ./parts my_recording.rrd
```

Each part is a separate recording.

## Compact a file

Many small chunks make a file slow to load and query.
`rerun rrd optimize` merges them into fewer, larger chunks:

```sh
rerun rrd optimize my_recording.rrd -o my_recording_optimized.rrd
```

The output also has a footer, which the Viewer needs to load large files on demand.
See [Optimize chunk count](optimize-chunks.md) and [Open recordings larger than RAM](../visualization/limit-ram.md#can-i-open-recordings-that-are-larger-than-ram).

## Merge files

`rerun rrd merge` writes the contents of several files into one file:

```sh
rerun rrd merge part1.rrd part2.rrd -o merged.rrd
```

Each recording stays a separate recording in the output.
Data from files that share the same application ID and recording ID ends up in the same recording.

To combine files with different IDs into a single recording, give them a common application ID and recording ID with `rerun rrd route`:

```sh
rerun rrd route --application-id my_app --recording-id my_recording part1.rrd part2.rrd -o combined.rrd
```

`rerun rrd route` does not decode the data, so it is fast, but it doesn't migrate or check the data either.

You can also open several files together without merging them: `rerun part1.rrd part2.rrd`.
Files with the same application ID and recording ID are shown as one recording.

## Append to a file, or rotate files

Rerun does not support appending to an existing `.rrd` file, or rotating files from the SDK.
`rr.save()` and `rr.FileSink` overwrite the file if it already exists.

Workarounds:

* Write a new file for each session or time span, with the same application ID and recording ID (the first argument and the `recording_id` argument of `rr.init`).
  Open them together, or combine them with `rerun rrd merge`.
  See [Shared recordings](shared-recordings.md).
* To rotate files, stop logging to one file and start a new one with `rr.save()` (or `rr.set_sinks`) on the same recording.

## Watch a file while it is written

The Viewer does not follow (tail) a file that is still being written.
To watch the data live and also save it, send it to both the Viewer and a file:

```python
rr.set_sinks(rr.GrpcSink(), rr.FileSink("my_recording.rrd"))
```

See [Sinks](../../concepts/logging-and-ingestion/sinks.md#multiple-sinks-tee-pattern).

Alternatively, stream to standard output and pipe it to the Viewer, which reads from standard input:

```sh
python my_script.py | rerun -
```

Your script needs to call `rr.stdout()` for this.
