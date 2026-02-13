---
title: Static data
order: 600
---


The Rerun SDK allows you to store data as _static_. Static data belongs to all timelines (existing ones, and ones not yet created) and shadows any temporal data of the same type on the same entity.

That is, any time you log static data to an entity path, all past, present and future temporal data on that same entity path and component is _semantically_ discarded in favor of the static one (which doesn't necessarily mean that it is _physically_ discarded, more on that below).


## How to store static data?

Internally, all data in Rerun is stored as chunks of columns. Specifically, each chunk holds zero or more time columns (the indices), and zero or more component columns (the data).
Static data is data that lives in a chunk whose set of time columns is the empty set.

The easiest way to create such chunks is by using the `log` family of methods, which exposes a `static` flag where appropriate:

snippet: concepts/static/log_static

The same can be achieved using the `send_columns` API by simply leaving the time column set empty:

snippet: concepts/static/send_static

(Using `send_columns` that way is rarely useful in practice, but is just a logical continuation of the data model.)


## When should I use static data?

There are two broad categories of situations where you'd want to use static data: scene setting and memory savings.


### Scene setting

Often, you'll want to store data that isn't part of normal data capture, but sets the scene for how it should be shown.
For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be static.

snippet: concepts/static/log_static

The alternative would be to log that data at the beginning of every relevant timeline, which can be very problematic as the set of timelines might not even be known before runtime.

Similarly, [coordinate systems](transforms.md) or [annotation context](../visualization/annotation-context.md) are typically stored as static.


### Memory savings

When you store _temporal_ data in Rerun, it is always appended to the existing dataset: there is no such thing as overwriting temporal data. The dataset only grows, it never shrinks.
To compensate for that, the Rerun viewer has a [garbage collection mechanism](../../howto/visualization/limit-ram.md) that will drop the oldest data from the store when memory becomes scarce.

For example, the following snippet stores 10 images at index `4` on the `frame` [timeline](timelines.md):

snippet: concepts/static/log_temporal_10x

All these images are actually stored, and all of them can be visualized in the viewer independently, even though they share the same index.

Contrary to temporal data, static data is **never** garbage collected… but it can actually be overwritten!
_Semantically_, only a single piece of static data can exist at a given time for a specific component on a specific entity.

In the following snippet, only the data from latest log call (in execution order) will be inspectable in the viewer:

snippet: concepts/static/log_static_10x

In practice, the Rerun datastore will rely on these semantics to physically drop the superfluous static data where possible, therefore drastically reducing memory costs. See ["Understanding storage costs"](#understanding-storage-costs) for more information.


## Understanding storage costs

In ["Memory savings"](#memory-savings), we mentioned that the following snippet _semantically_ stores a single image:

snippet: concepts/static/log_static_10x

How these semantics actually translate to physical storage depends on the context.


### In recordings

Rerun recordings (`.rrd` files) are just streams of binary messages: they have no semantics whatsoever, therefore they don't know what static means and can't do anything about it.

If you were to log the snippet above to a file (using e.g. `rr.save()`), you'd find that the recording does in fact contains your 10 images.

If you wanted the recording file itself to only contain a single static value, you would need to either:
* Stream the data to the viewer, and then save the recording directly out of the viewer using `Menu > Save recording` (or the equivalent palette command).
* Manually recompact your recording using the [Rerun CLI](../../reference/cli.md#rerun-rrd-compact) so that the data overwrite semantics can get appropriately applied, e.g.: `rerun rrd compact -o compacted.rrd myrecording.rrd`.


### In the viewer

The data store that backs the Rerun viewer natively understands these temporal/garbage-collected vs. static/overwritten semantics.

If you were to log the snippet above directly to the Rerun viewer (using e.g. `rr.connect_grpc()`), you'd notice that the viewer's memory usage stays constant: the data is automatically being overwritten as new updates come in.
For data where you don't need to keep track of historical values, this effectively to logs its new values indefinitely.

In the following example, you can see our [face tracking example]() indefinitely tracking my face while maintaining constant memory usage by logging all data as static:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/tutorials/infinite_face_tracking.mp4" type="video/mp4" />
</video>
