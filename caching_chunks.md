:warning: **THIS IS A WORK IN PROGRESS -- I DO NOT EXPECT IT TO MAKE MUCH SENSE.** :warning:

Rethinking query semantics for aggregated caching
=================================================

We want to be able to cache aggregations of Chunks (where the aggregated data can take any form, as long as it is sharded along its source *primary Chunks*), *regardless of how these chunks came together*.
This will be a very important part of our upcoming [Chunk processing primitives](https://github.com/rerun-io/rerun/issues/8221), which are themselves an important of [making Rerun work with many entities](https://github.com/rerun-io/rerun/issues/8233).

Specifically, we want to be able to cache Chunks that were _range-zipped_ together.
[`RangeZip` is a well-defined, deterministic operation](https://github.com/rerun-io/rerun/blob/main/crates/store/re_query/src/range_zip/generated.rs) that we do all over the place, any time we need to interlace data from multiple streams together.
It is the core joining operation in Rerun: grab columnar data from different components, and zip them together over time (where "time" really means "index" and is defined as a tuple of `(Timeline, RowId)` [1]).

Unfortunately, as of today, range-zipping will seemingly behave differently depending on how the data was queried / put together: iterating raw Chunks vs. `AggregatedLatestAt` vs. `AggregatedRange`, etc.
How is that even possible that the origin of the data impacts this operation at all, and what can we do to get out of this mess?

Fixing all these problems is a pre-requisite in order to be able to cache aggregated Chunks.

This document summarizes everything there is to know about existing query pitfalls, and proposes semantic changes in order to make aggregated caching attainable.
It also implicitly names a bunch of concepts that were until now anonymous, and defines a syntax to talk about Chunks, and aggregations of Chunks.

---

[1] Well, sometimes... Read on.


## Background: range-zipping Chunks

Say you have the following two chunks:
```rust
  frame_nr   component
  --------   ---------

  CHUNK C0
    #0       Radius(1.0)
    #15      Radius(2.0)

  CHUNK C1
    #10      Position(1, 1, 1)
    #20      Position(2, 2, 2)
```

Range-zipping is the operation that will merge these two streams of data into a single logical stream.
It is a form of aggregation, and it's used by all of our visualizers in order to merge the many different streams of components they get from executing `LatestAt` and/or `Range` queries.
I will refer to a series of `LatestAt`/`Range` queries followed by a `RangeZip` as `AggregatedLatestAt`/`AggregatedRange` from now on. More on these later.

Range-zipping requires a point-of-view, from which the joining is performed. The resulting stream of data will yield one entry for each index where the point-of-view was updated, in any of the source streams.
As of today, we only ever rely on single point-of-views, though that is expected to change in the future.

There are two unique components in this specific example, and as such there are two possible `RangeZip` operations possible:
* `RangeZip(PoV: Position, chunks: [C0, C1])`
* `RangeZip(PoV: Radius,   chunks: [C0, C1])`

`RangeZip(PoV: Position, chunks: [C0, C1])` yields:
```rust
  frame_nr    position (pov)      radius
  --------    --------------      ------

CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))
    #10       Position(1, 1, 1)   Radius(1.0)
    #20       Position(2, 2, 2)   Radius(2.0)
```

`RangeZip(PoV: Radius,   chunks: [C0, C1])` yields:
```rust
  frame_nr    radius        position (pov)
  --------    ------        --------------

CHUNK(RangeZip(PoV: Radius, chunks: [C0, C1]))
    #0        Radius(1.0)   <None>
    #15       Radius(2.0)   Position(1, 1, 1)
```

> [!NOTE]
> I've omitted `RowId`s in this example for brevity's sake. We will come back to `RowId`s later on.


### :warning: Pitfall: `RangeZip` is **not** globally-defined!

TODO: aka susceptible to inner filtering

Range-zipping is *not* a globally-defined operation -- it can and will yield contradicting results if the data streams get offset or filtered in any way.

Consider the two Chunks from the example above once again, but this time let's assume they have been applied a `.filtered(#5..=#20)`:
```rust
  frame_nr   component
  --------   ---------

  CHUNK C0.filtered(#5..=#20)
    #15      Radius(2.0)

  CHUNK C1.filtered(#5..=#20)
    #10      Position(1, 1, 1)
    #20      Position(2, 2, 2)
```

`RangeZip(PoV: Position, chunks: [C0.filtered(#5..=#20), C1.filtered(#5..=#20)])` now yields:
```diff
RangeZip(PoV: Position, chunks: [C0.filtered(#5..=#20), C1.filtered(#5..=#20)])
  frame_nr    position (pov)      radius
  --------    --------------      ------
-   #10       Position(1, 1, 1)   Radius(1.0)
+   #10       Position(1, 1, 1)   <None>
    #20       Position(2, 2, 2)   Radius(2.0)
```

The same operation on the same Chunks yielded the same rows (indices `#10` and `#20`), but with different data!

> [!CAUTION]
> Because `RangeZip` is not globally-defined, it is impossible to cache the aggregated results of two or more Chunks, as that might or might not yield the correct results depending on which time range the querier is interested in.

This is the source of a lot of complexity, and has caused numerous bugs, particularly around caching.
The obvious fix is to always apply range-zipping on raw, unaltered Chunks... but that's not quite good enough: what about the Chunks you don't necessarily know about?

### :warning: Pitfall: `RangeZip` is **not** bootstrapped

TODO: aka susceptible to outer filtering

Say you have the following two chunks:
```rust
  frame_nr   component
  --------   ---------

  CHUNK C0
    #15      Radius(1.0)

  CHUNK C1
    #10      Position(1, 1, 1)
    #20      Position(2, 2, 2)
```

We've already established what range-zipping those two Chunks look like:
```rust
  frame_nr    position (pov)      radius
  --------    --------------      ------

CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))
    #10       Position(1, 1, 1)   <None>
    #20       Position(2, 2, 2)   Radius(2.0)
```

But what if there is yet another Chunk out there such that:
```rust
  frame_nr   component
  --------   ---------

  CHUNK C2
    #0       Radius(0.0)
```

Obviously, taking that third Chunk into account during range-zipping will change the aggregated output.
This has nothing to do with `RangeZip` nor Chunks though:
* `CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))` is deterministic.
* `CHUNK(RangeZip(PoV: Position, chunks: [C0, C1, C2]))` is also deterministic.
* `CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))` != `CHUNK(RangeZip(PoV: Position, chunks: [C0, C1, C2]))`, which is fine.

This is really just another manifestation of "`RangeZip` is not globally-defined", just one layer above (i.e. filtering whole Chunks vs. filtering rows of Chunks), so why do we care?
The problem is that which Chunks you are or are not aware of during zipping is highly dependent on the kind of query you run... In fact, even the data within these Chunks might change across queries.

> [!CAUTION]
> Because `RangeZip` isn't globally-defined, and because different queries can return different Chunks, and even modify the data in those Chunks, caching Chunks across different queries is also impossible.


## Background: aggregated queries

We define an *aggregated query* as a series of single-component `LatestAt` and/or `Range` queries (reminder: there is no such thing as a multi-component `LatestAt`/`Range`), whose results (Chunks) are then aggregated using `RangeZip`.

To avoid confusion, I will refer to single-component/low-level queries as `LatestAt` and `Range`, whereas multi-component/aggregated/range-zipped/high-level queries will be referred to as `AggregatedLatestAt` and `AggregatedRange`, respectively.

Aggregated queries are the core building block of our visualizers:
* Because they rely on `RangeZip`, they inherit all the pitfalls above.
* Because they rely on `LatestAt` and `Range` queries, they also inherit from their semantic quirks.
* Because they mix all of these things, they create their very own annoying pitfalls.

In particular, there are two very specific aggregated-query semantic pitfalls that make it impossible to cache range-zipped Chunks at the moment:
* Local vs. global determinism
* Peeking into the future


### :warning: Pitfall: Local vs. global determinism

Our `AggregatedLatestAt` queries are globally deterministic, whereas our `AggregatedRange` queries are merely locally deterministic.

What that means in pratice is that for a given _fixed, immutable dataset_ (i.e. no Chunks are actively being added nor removed from the store), the results you get at timestamp `ts_result`, for a `LatestAt` query at timestamp `ts_query`, are completely deterministic, regardless of what `ts_query` you use.
This is _not_ true for `Range` queries.

Say you have the following data residing in storage:
```rust
CHUNK_STORE
  frame_nr   component
  --------   ---------

  CHUNK C0
    #0       Radius(1.0)
    #15      Radius(2.0)

  CHUNK C1
    #10      Position(1, 1, 1)
    #20      Position(2, 2, 2)
```

Any `AggregatedLatestAt` query executed for `#10 <= t < #15` will always yield the same results:
```rust
* AggregatedLatestAt(at: #10, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #11, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #12, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #13, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #14, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
```

Now consider an `AggregatedRange` query -- it only takes two examples to see it all fall apart:
```rust
* `AggregatedRange(range: #0..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
* `AggregatedRange(range: #1..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), None)]
```

Both `AggregatedRange` queries yield data from the same index (`#10`), but with different values: `AggregatedLatestAt` queries are globally deterministic, whereas `AggregatedRange` queries are locally deterministic.
Nothing surprising: it's a straightforward manifestation of `RangeZip`s pitfalls, just in the context of an `AggregatedRange` query.

This is a natural and easy to understand consequence of `AggregatedRange` queries not being bootstrapped -- but you can extrapolate the effect that something like this has on caching.
In fact, this is one of the main reason why our range-query cache doesn't actually cache queries at all, but rather the underlying Chunks necessary to compute the results: caching actual range queries would be extremely painful (we know, we've been there).

> [!CAUTION]
> Consider what happens when range-zipping the two Chunks above:
> ```rust
>   frame_nr   position (pov)      radius
>   --------   --------------      ------
>
> CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))
>     #10      Position(1, 1, 1)   Radius(1.0)
>     #20      Position(2, 2, 2)   Radius(2.0)
> ```
>
> Notice that [`#10: (Position(1, 1, 1), None)`], which is a possible outcome of running a `Range` query on the dataset, is not a possible value when blindly zipping the Chunks together without further context.
> Aggregated caching is therefore impossible.


### :warning: Pitfall: Peeking into the intra-timestamp future

`AggregatedLatestAt` queries break the rules of indexing by peeking into the future, whereas our `AggregatedRange` queries don't.

Say you have the following data residing in storage:
```rust
CHUNK_STORE
  frame_nr   row_id   component
  --------   ------   ---------

  CHUNK C0
    #0       101      Radius(1.0)
    #10      1099     Radius(2.0)

  CHUNK C1
    #10      1001     Position(1, 1, 1)
    #20      2001     Position(2, 2, 2)
```

If you were to run a `AggregatedLatestAt` query on top of that data at `t=#10`, you'd get the following (hint: take a close look at the `Radius`):
```rust
* AggregatedLatestAt(at: #10, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(2.0))
```

Compare that to an `AggregatedRange` query (hint: look at the `Radius`):
```rust
* AggregatedRange(range: #0..#11, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
```

What the `AggregatedLatestAt` query is doing is technically illegal: somehow we're saying that our `Position` at index `(#10, 1001)` has an associated `Radius` at index `(#10, 1099)` -- that is, from the future.

Of course this is no mistake, the viewer would be pretty much unusable otherwise (imagine having to meticulously execute your log calls in the perfect order when trying to log multiple components to the same `frame_nr`).
This is very much intended behavior, and the only reason it works at all is because we explicitly monkey-patch the `RowId`s at the last second in the visualizers to let them think that the data is _not_ coming from the future:
https://github.com/rerun-io/rerun/blob/94d545b52bc8039332281c17c8b5773140caff49/crates/viewer/re_space_view/src/results_ext.rs#L368-L373

> [!CAUTION]
> Consider what happens when range-zipping the two Chunks above:
> ```rust
>   frame_nr   position (pov)      radius
>   --------   --------------      ------
>
> CHUNK(RangeZip(PoV: Position, chunks: [C0, C1]))
>     #10       Position(1, 1, 1)   Radius(1.0)
>     #20       Position(2, 2, 2)   Radius(2.0)
> ```
> 
> Notice that `#10: (Position(1, 1, 1), Radius(2.0))`, which is a possible outcome of running an `AggregatedLatestAt` query on the dataset, is not a possible value when blindly zipping the Chunks together without further context.
> This in turn makes aggregated caching impossible.


### :warning: Pitfall: Peeking into the intra-timestamp future

TODO: All of the above is even true at the inter-timestamp level. This is closely related to the complicated rules around bootstrapping.


## Background: Summary of existing queries

There exists 5 different queries used within Rerun:
* `LatestAt`: A low-level latest-at query.
* `Range`:  A low-level range query.
* `AggregatedLatestAt`: A multi-component latest-at query, aggregating results from many `LatestAt`s (for implementing Visualizers).
* `AggregatedRange`: A multi-component range query, aggregating results from many `Range`s (for implementing Visualizers).
* `Dataframe`: A high-level dataframe query.

Query kind         | Entities | Components | Yield semantics                      | Intra-timestamp semantics   | Bootstrapping                 
------------------ | -------- | ---------- | ------------------------------------ | --------------------------- | ------------------------------
LatestAt           | Single   | Single     | Per timestamp, primary only          | Yield latest only           | Global `LatestAt` (implied)   
Range              | Single   | Single     | Per index, primary only              | Yield all                   | None                          
AggregatedLatestAt | Single   | Many       | Per timestamp, primary only          | Yield latest only           | Global `LatestAt` (implied)   
AggregatedRange    | Single   | Many       | Per index, primary only              | Yield all                   | None                          
Dataframe          | Many     | Many       | Per timestamp, any component         | Yield latest only           | Optionally: Global `LatestAt` 

_(Continued)_
Query kind         | Densification                  | Join semantics           | Used in
------------------ | ------------------------------ | ------------------------ | -------
LatestAt           | N/A                            | N/A                      | Ad-hoc viewer queries, implementation of `AggregatedLatestAt` & `Dataframe`
Range              | N/A                            | N/A                      | implementation of `AggregatedRange`
AggregatedLatestAt | N/A                            | Index-patched `RangeZip` | Ad-hoc viewer queries, Visualizers
AggregatedRange    | Accumulated                    | Vanilla `RangeZip`       | Visualizers
Dataframe          | Optionally: Global `LatestAt`  | Per-timestamp join       | Dataframe APIs (and likely some visualizers in the future)

Terminology:
* `Query kind`: which query are we talking about?
  * `LatestAt`: A low-level latest-at query.
  * `Range`:  A low-level range query.
  * `AggregatedLatestAt`: A multi-component latest-at query, aggregating results from many `LatestAt`s (for implementing Visualizers).
  * `AggregatedRange`: A multi-component range query, aggregating results from many `Range`s (for implementing Visualizers).
  * `Dataframe`: A high-level dataframe query.
* `Entities`: how many entities can be queried at once?
* `Components`: how many components can be queried at once?
* `Yield semantics`: how often does the query yields a new row?
  * `Per timestamp, primary only`: for every unique timestamp for which there is at least one row of data where the primary component is non-null.
  * `Per index, primary only`: for every unique index (`(timestamp, rowid)`) for which there is at least one row of data where the primary component is non-null.
  * `Per timestamp, primary & secondaries`: for every unique timestamp for which there is at least one row of data where either the primary or any secondary components are non-null.
* `Intra-timestamp semantics`: what are the semantics used when the data contains multiple rows for a single timestamp?
  * `Yield latest only`: only yield the latest value for that timestamp, accorcding to the full index (i.e. time + rowid).
  * `Yield all`: yield all values available for that timestamp.
* `Bootstrapping`: what initial state is used to bootstrap the results?
  * `None`: none.
  * `Global LatestAt`: build up initial state by running global scope `LatestAt` queries ("(implied)" means that the query bootstraps itself by its very nature).
* `Densification`: how are empty cells about to be yielded filled with data?
  * `Accumulated`: data is accumulated from previous iterations.
  * `Global LatestAt`: data is fetched via a global scope `LatestAt`.
* `Join semantics`: how are multiple component streams joined together into one?
  * `Vanilla RangeZip`: our well-defined [range-zip](https://github.com/rerun-io/rerun/blob/main/crates/store/re_query/src/range_zip/generated.rs), without any further shenanigans.
  * `Index-patched RangeZip`: Like `RangeZip`, but all the indices are [monkey-patched as `(TimeInt::STATIC, RowId::ZERO)`](https://github.com/rerun-io/rerun/blob/94d545b52bc8039332281c17c8b5773140caff49/crates/viewer/re_space_view/src/results_ext.rs#L368-L373) ( :warning: ), effectively ignoring row-ordering.
  * `Per-timestamp join`: dataframe specific code that joins rows with same exact same timestamp, without cross-timestamp accumulation.
* `Used in`: where is this used?

By now you should have enough background to understand what all of these means, and be able to figure out how all of these things might or might not interact with aggregated caching.

> [!CAUTION]
> Query semantics are way too complicated, making it very hard to reason about even for core Rerun team members.

> [!CAUTION]
> Query semantics vary in small but important ways across our different queries, affecting their Chunks output, and therefore rendering aggregated caching close to impossible.


## Proposal

TODO:
* Simplify queries to reduce the insanity and differences in semantics.
* Simplify queries in order to make aggregated chunks cacheable.

TODO: we will leave the dataframe APIs out of it for now.
TODO: should overlap be a column?


Move from this:

Query kind         | Entities | Components | Yield semantics                      | Intra-timestamp semantics   | Bootstrapping                 
------------------ | -------- | ---------- | ------------------------------------ | --------------------------- | ------------------------------
LatestAt           | Single   | Single     | Per timestamp, primary only          | Yield latest only           | Global `LatestAt` (implied)   
Range              | Single   | Single     | Per index, primary only              | Yield all                   | None                          
AggregatedLatestAt | Single   | Many       | Per timestamp, primary only          | Yield latest only           | Global `LatestAt` (implied)   
AggregatedRange    | Single   | Many       | Per index, primary only              | Yield all                   | None                          

_(Continued)_
Query kind         | Densification                  | Join semantics           | Used in
------------------ | ------------------------------ | ------------------------ | -------
LatestAt           | N/A                            | N/A                      | Ad-hoc viewer queries, implementation of `AggregatedLatestAt` & `Dataframe`
Range              | N/A                            | N/A                      | implementation of `AggregatedRange`
AggregatedLatestAt | N/A                            | Index-patched `RangeZip` | Ad-hoc viewer queries, Visualizers
AggregatedRange    | Accumulated                    | Vanilla `RangeZip`       | Visualizers

to this (changes indicated in ❗**bold**):

Query kind         | Entities | Components | Yield semantics                      | Intra-timestamp semantics   | Bootstrapping                 
------------------ | -------- | ---------- | ------------------------------------ | --------------------------- | ------------------------------
LatestAt           | Single   | Single     | Per timestamp, primary only          | Yield latest only           | Global `LatestAt` (implied)   
Range              | Single   | Single     | ❗**Per timestamp, primary only**    | ❗**Yield latest only**     | None
AggregatedLatestAt | Single   | Many       | Per timestamp, primary only          | Yield latest only           | ❗**Index-patched global `LatestAt`**       
AggregatedRange    | Single   | Many       | ❗**Per timestamp, primary only**    | ❗**Yield latest only**     | ❗**Index-patched global `LatestAt`**       

_(Continued)_
Query kind         | Densification                  | Join semantics           | Used in
------------------ | ------------------------------ | ------------------------ | -------
LatestAt           | N/A                            | N/A                      | Ad-hoc viewer queries, implementation of `AggregatedLatestAt` & `Dataframe`
Range              | N/A                            | N/A                      | implementation of `AggregatedRange`
AggregatedLatestAt | N/A                            | ❗**Vanilla `RangeZip`** | Ad-hoc viewer queries, Visualizers
AggregatedRange    | ❗**N/A**                      | Vanilla `RangeZip`       | Visualizers


TODO: should all of them be "Index-patched bootstrapped"?

TL;DR
* Make `Range` semantics much closer to `LatestAt`'s.
* Get rid of per-index yields altogether.

---

TODO

### :warning: Pitfall: Local vs. global determinism

Say you have the following data residing in storage:
```rust
CHUNK_STORE
  frame_nr   component
  --------   ---------

  CHUNK C0
    #0       Radius(1.0)
    #15      Radius(2.0)

  CHUNK C1
    #10      Position(1, 1, 1)
    #20      Position(2, 2, 2)
```

Index-patched bootstrap:
```rust
frame_nr   component
--------   ---------

CHUNK CB0
  #STATIC  Radius(1.0)

CHUNK CB1
  #STATIC  Radius(1.0)
```

Any `AggregatedLatestAt` query executed for `#10 <= t < #15` will still always yield the same results:
```rust
* AggregatedLatestAt(at: #10, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #11, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #12, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #13, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
* AggregatedLatestAt(at: #14, comps: [Position, Radius]) = #10: (Position(1, 1, 1), Radius(1.0))
```

But now, so will an `AggregatedRange` query:
```rust
* `AggregatedRange(range: #0..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
* `AggregatedRange(range: #1..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
* `AggregatedRange(range: #2..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
* `AggregatedRange(range: #3..#12, PoV: Position, comps: [Radius]) = [#10: (Position(1, 1, 1), Radius(1.0))]
```

---

TODO: okay but how does that help?
TODO: so then, we always bootstrap, and specifically we always bootstrap with a patch
TODO: how does that solve the problems above?
TODO: ok but what happens with future peeking non-sense?
TODO: we should always indicate the PoV everywhere
TODO: why does range zip even need a pov???
