---
title: Overview
order: 50
---

Internally, the [`DataLoader`](https://docs.rs/re_data_loader/latest/re_data_loader/trait.DataLoader.html) trait takes care of loading files into the Viewer and/or SDK.

There are 3 broad kinds of `DataLoader`s: _builtin_, _external_ and _custom_.
_External_ and _custom_ are the two ways of extending the file loading system that we'll describe below.

When a user attempts to open a file in the Viewer/SDK, **all** known `DataLoader`s are notified of the path to be opened, unconditionally.
This gives `DataLoader`s maximum flexibility to decide what files they are interested in, as opposed to e.g. only being able to look at a file's extension.

Once notified, a `DataLoader` can return a [`DataLoaderError::Incompatible`](https://docs.rs/re_data_loader/latest/re_data_loader/enum.DataLoaderError.html#variant.Incompatible) error to indicate that it doesn't support a given file type.
If, and only if, all loaders known to the Viewer/SDK return an `Incompatible` error code, then an error message is shown to the user indicating that this file type is not (_yet_) supported.

In these instances of unsupported files, we expose two ways of implementing and registering your `DataLoader`s, explained below.

### External data-loaders

The easiest way to create your own `DataLoader` is by implementing what we call an "external loader": a stand alone executable written in any language that the Rerun SDK ships for. Any executable on your `$PATH` with a name that starts with `rerun-loader-` will be treated as a `DataLoader`.

This executable takes a file path as a command line argument and outputs Rerun logs on `stdout`.
It will be called by the Rerun Viewer/SDK when the user opens a file, and be passed the path to that file.
From there, it can log data as usual, using the [`stdout` logging sink](../../../reference/sdk/operating-modes.md#standard-inputoutput).

The Rerun Viewer/SDK will then automatically load the data streamed to the external loader's standard output.

<picture>
  <img src="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/data-loader-external-overview/97e978000c709b78290f50d52c229a91f7543648/1200w.png">
</picture>

Like any other `DataLoader`, an external loader will be notified of all file openings, unconditionally.
To indicate that it does not support a given file, the loader has to exit with a [dedicated status code](https://docs.rs/rerun/latest/rerun/constant.EXTERNAL_DATA_LOADER_INCOMPATIBLE_EXIT_CODE.html).

When the Viewer and/or SDK executes an external loader, it will pass to it a set of recommended settings in the form of CLI parameters (in addition to the file path to be loaded, which is passed as the one and only positional argument):

* `--application-id <application_id>`

    The recommended `ApplicationId` to log the data to.

* `--opened-application-id <opened_application_id>` (optional)

    The `ApplicationId` that is currently opened in the viewer, if any.

* `--recording-id <store_id>`

    The recommended `RecordingId` to log the data to.

    Log data to this recording if you want it to appear in a new recording shared by all
    data-loaders for the current loading session.

* `--opened-recording-id <opened_store_id>` (optional)

    The `RecordingId` that is currently opened in the viewer, if any.

* `--entity-path-prefix <entity_path_prefix>` (optional)

    Recommended prefix to prepend to all entity paths.

* `--static` (optional)

    The data is expected to be logged as static.

* `--time_sequence <timeline1>=<seq1> <timeline2>=<seq2> …` (optional)

    The data is expected to be logged at these specific sequence times.

* `--time_duration_nanos <timeline1>=<duration1> <timeline2>=<duration2> …` (optional)

    The data is expected to be logged at these specific duration times.

    The timestamps are expected to be in nanoseconds: use `rr.set_time_duration_nanos` (Python) / `RecordingStream::set_time_duration_nanos` (C++, Rust) appropriately.

* `--time_timestamp_nanos <timeline1>=<timestamp1> <timeline2>=<timestamp2> …` (optional)

    The data is expected to be logged at these specific timestamp times.

    The timestamps are expected to be in nanoseconds since Unix epoch: use `rr.set_time_timestamp_nanos` (Python) / `RecordingStream::set_time_timestamp_nanos` (C++, Rust) appropriately.

Check out our examples for [C++](https://github.com/rerun-io/rerun/tree/main/examples/cpp/external_data_loader), [Python](https://github.com/rerun-io/rerun/tree/main/examples/python/external_data_loader) and [Rust](https://github.com/rerun-io/rerun/tree/main/examples/rust/external_data_loader) that cover every steps in details.

### Custom Rust data-loaders

Another Rust-specific approach is to implement the `DataLoader` trait yourself and register it in the Rerun Viewer/SDK.

To do so, you'll need to import `rerun` as a library, register your `DataLoader` and then start the Viewer/SDK from code.

Check out our [example](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_data_loader) that cover all these steps in details.
