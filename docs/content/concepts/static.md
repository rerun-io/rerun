---
title: Static data
order: 450
---

## What is static data?

The Rerun SDK offers the possibility to store data as _static_.

Static data belongs to all timelines (existing ones, and ones not yet created) and shadows any temporal data of the same type on the same entity.
That is, any time you log static data to an entity path, all past, present and future temporal data on that same entity path is _semantically_ discarded (which doesn't necessarily mean that it is _physically_ discarded, more on that below).

This can be achieved using the `log` family of methods, by toggling the `static` flag where appropriate:

snippet: concepts/static/log_static

Or using `send_columns`, by leaving the time column data empty:

snippet: concepts/static/send_static

## When should I use static data?

There are two broad categories of use cases for static data: scene setting, and memory optimizations.

### Scene setting

Often, you'll want to store data that isn't part of normal data capture, but sets the scene for how it should be shown.
For instance, if you are logging cars on a street, perhaps you want to always show a street mesh as part of the scenery, and for that it makes sense for that data to be static.

The alternative would be to log that data at the beginning of every relevant timeline, which can be very problematic as the set of timelines might not even be known before runtime.

Similarly, [coordinate systems](spaces-and-transforms.md) or [annotation context](annotation-context.md) are typically static.


* How much space does static data take:
  * In the viewer?
  * In a recording file?
* How can one compact static data in a recording?


## Interaction with data APIs:

### Dataframe APIs

### Low-level APIS: LatestAt

### Low-level APIS: Range

## Overwrite semantics






> Even though the data is now static=True, when I re-open the visualizer it plays back the recording in fast-forward. Which means, it needs to have stored the data somewhere. 
> 
> That is expected: Rerun recordings are just streams of binary messages, they have no semantics whatsoever, therefore they don't know what static means and can't do anything about it.
> The datastore that lives in the viewer, on the other hand, understand these semantics and can apply garbage collection rules accordingly.
> 
> If you want the recording file itself to only contain a single static value, you need to either:
> Stream the data to the viewer, and then save the recording directly out of the viewer using Menu > Save recording.
> Manually recompact your recording using the Rerun CLI so that GC gets applied: rerun rrd compact -o compacted.rrd myrecording.rrd
> [7:25 PM]Clement:
> When looking at htop, I can see how my python app continues to accumulate memory.
> I am testing this now locally with a larger sensor, it even seems worse than before: my memory usage for the python process goes up without any seeming limit/flush. I'm now at 2.2 Gbyte and still increasing.
> 
> I don't have enough context to be able to tell what you're looking at here.
> Static semantics are completely unrelated to client-side memory usage though.
