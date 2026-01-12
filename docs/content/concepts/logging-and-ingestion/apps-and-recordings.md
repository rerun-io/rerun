---
title: Recordings
order: 100
---

Recordings are the core abstraction for organizing data in Rerun.

A Recording is a semantic collection of data with an associated _Recording ID_ (which is just another name for a UID). That's it.

Recordings are a _logical abstraction_, not a physical one: a recording is not confined to a specific file, or folder, or database, or whichever physical storage you might think of.
Similarly, there is no such thing as "closing" a recording: as long as there exists or will exist a system somewhere that is capable of producing _chunks_ of data, and tagging these chunks with the appropriate _Recording ID_, then that recording is effectively still growing. Whether that happens today, tomorrow, or in some distant future.

This design naturally allows for both the production and the storage of recordings to be horizontally distributed:
* Production can be handled by multiple producers that all log data to the same _Recording ID_, independently.
* Storage can be sharded over multiple independent files (or any other storage medium).

You can learn more about sharding in the [dedicated documentation page](../../howto/logging/shared-recordings.md).

In practice, most Rerun recordings are encoded in binary files with the `.rrd` extension by default. This is our basic storage solution for recordings, which is specifically designed for streaming use cases (i.e. `.rrd` files do not offer random-access to the data within).
Note that [blueprints](../visualization/blueprints.md) are recordings too, and by convention are stored in binary `.rbl` files.


## Application IDs

Rerun recordings have an extra piece of metadata associated with them in addition to their _Recording ID_: an _Application ID_. _Application IDs_ are arbitrary user-defined strings.

When you initialize the Rerun logging SDK, you need to set an _Application ID_.

snippet: tutorials/custom-application-id

The Rerun viewer will store your blueprint based on this _Application ID_.

This means that you can run your app and set up the viewer to your liking, and then when you run the app again the Rerun viewer will remember how you set up your Views etc.
Different recordings (i.e. different _Recording IDs_) will share the same blueprint as long as they share the same _Application ID_.

Check out the API to learn more about SDK initialization:
* [🐍 Python](https://ref.rerun.io/docs/python/stable/common/initialization_functions/#rerun.init)
* [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html#method.new)
* [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#abda6202900fa439fe5c27f7aa0d1105a)


## Recording IDs in practice

Each time you start logging using Rerun, a random _Recording ID_ is generated. For instance, each `.rrd` file will have a unique _Recording ID_.

This means you can have multiple recordings with different Recording IDs sharing the same application ID.

If you want to log from multiple processes and want all the log data to show up together in the viewer, you need to make sure all processes use the same Recording ID.
