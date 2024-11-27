:warning: **THIS IS A WORK IN PROGRESS -- IT DOES NOT NECESSARILY MAKE ANY SENSE. I'LL REMOVE THIS DISCLAIMER ONCE IT FINALLY DOES.** :warning:

## Context

We've had a long discussion with @Wumpf specifically regarding the caching aspect.
Chunk Processors will require us to cache aggregated data (range-zipped data, to be exact), which is a first for us.

The following proposal first requires [rethinking our query semantics](https://github.com/rerun-io/rerun/issues/8283), which makes caching aggregated Chunks semantically possible in the first place.
You should familiarize yourself with that document before continuing, as well as all the previous posts in this thread.

To make caching of aggregated Chunks possible, we will need to introduce two new types of queries: `UnslicedAggregatedLatestAt` and `UnslicedAggregatedRange`.
`UnslicedAggregatedLatestAt` and `UnslicedAggregatedRange` are exactly what they sound like -- the behave like their `AggregatedLatestAt`/`AggregatedRange` counterparts, but never slice the results down to the exact bounds.

To implement `UnslicedAggregatedLatestAt` and `UnslicedAggregatedRange`, we first need to implement `UnslicedLatestAt` and `UnslicedRange`.
So all in all, we add 4 more queries: `UnslicedLatestAt`, `UnslicedRange`, `UnslicedAggregatedLatestAt`, `UnslicedAggregatedRanged`. Fortunately these are all almost identical to their sliced counterparts.
(Feel free to propose a better name than `Unsliced` :melting_face: ).

It is up to the caller to then slice those aggregations further. This allows for caching any aggregation of data at the Chunk-level, which in turns allows for a lot of optimization (see previous comments in this issue).


## New queries

For the rest of this section, assume the following store:
```rust
CHUNK_STORE
  frame_nr   component
  --------   ---------

  CHUNK CR1
    #0       Radius(0.0)
    #10      Radius(10.0)

  CHUNK CR2
    #5       Radius(5.0)
    #30      Radius(30.0)

  CHUNK CP1
    #10      Position(10, 10, 10)
    #20      Position(20, 20, 20)

  CHUNK CP2
    #0       Position(0, 0, 0)
    #30      Position(30, 30, 30)
```


### `UnslicedLatestAt`

`UnslicedLatestAt` is a `LatestAt` query that always returns the raw, unaltered Chunk that contains the result, as opposed to a unit-slice of it that only contains the row we're interested in.
I.e. `LatestAt` can be implemented in terms of `UnslicedLatestAt` simply by slicing down the resulting Chunk.

Consider e.g. `LatestAt(at: #15, comp: Position)` vs. `UnslicedLatestAt(at: #15, comp: Position)`.

`LatestAt(at: #15, comp: Position)`:
```rust
  frame_nr   component
  --------   ---------

CHUNK(LatestAt(at: #15, comp: [Position]))
    #15      Position(10, 10, 10)
```

`UnslicedLatestAt(at: #15, comp: Position)`:
```rust
  frame_nr   component
  --------   ---------

CHUNK(UnslicedLatestAt(at: #15, comp: [Position])) == {CP1}
  CHUNK CP1
    #10      Position(10, 10, 10)
    #20      Position(20, 20, 20)
```

> [!NOTE]
> Similar to `LatestAt`, `UnslicedLatestAt` is guaranteed to return a single Chunk, always -- even in case of overlaps (such as demonstrated in this example).


### `UnslicedRange`

`UnslicedRange` is a `Range` query that always returns the raw, unaltered Chunks that contain the results, as opposed to sliced-down Chunks that only contain the rows we're interested in.
I.e. `Range` can be implemented in terms of `UnslicedRange` by slicing down the resulting Chunks.

Consider e.g. `Range(in: #5..=#30, comp: Position)` vs. `UnslicedRange(in: #5..=#30, comp: Position)`.

`Range(in: #5..=#30, comp: [Position])`:
```rust
  frame_nr   component
  --------   ---------

CHUNK(Range(in: #5..=#25, comp: Position))
    #10      Position(10, 10, 10)
    #20      Position(20, 20, 20)
```

`UnslicedRange(in: #5..=#25, comp: [Position])`:
```rust
  frame_nr   component
  --------   ---------

CHUNK(UnslicedRange(in: #5..=#25, comp: Position)) == {CP1, CP2}
  CHUNK CP1
    #10      Position(10, 10, 10)
    #20      Position(20, 20, 20)

  CHUNK CP2
    #0       Position(0, 0, 0)
    #30      Position(30, 30, 30)
```

> [!NOTE]
> Similar to `Range`, `UnslicedRange` might return any number of Chunks, depending on overlap.


### `UnslicedAggregatedLatestAt`

`UnslicedAggregatedLatestAt` is an `AggregatedLatestAt` query that range-zips the raw, unaltered Chunks that contain the results, as opposed to range-zipping the unit-sliced Chunks with only the rows we're interested in.
I.e. `AggregatedLatestAt` can be implemented in terms of `UnslicedAggregatedLatestAt` by slicing down the resulting aggregated Chunk.

Consider e.g. `AggregatedLatestAt(at: #15, comps: [Position, Radius])` vs. `UnslicedAggregatedLatestAt(at: #15, comps: [Position, Radius])`.

`AggregatedLatestAt(at: #15, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(AggregatedLatestAt(at: #15, comps: [Position, Radius]))
    #15      Position(10, 10, 10)    Radius(10.0)
```

`UnslicedAggregatedLatestAt(at: #15, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(UnslicedAggregatedLatestAt(at: #15, comps: [Position, Radius])) == RangeZip(PoV: Position, chunks: [CP1, CR1])
  // Primary: CP1
  // Dependencies: [CR1]
  CHUNK(RangeZip(PoV: Position, chunks: [CP1, CR1]))
    #10      Position(10, 10, 10)    Radius(10.0)
    #20      Position(20, 20, 20)    Radius(10.0)
```

> [!NOTE]
> Note that there doesn't exist a query (whether it's an `AggregatedLatestAt` or an `AggregatedRange`) that you could run on the underlying store that would yield different data for any of the indices present in this aggregated Chunk. I.e. we can cache this.

As you can see from looking at the results, `UnslicedAggregatedLatestAt` also reports the dependency graph of its aggregated results.


### `UnslicedAggregatedRange`

`UnslicedAggregatedRange` is an `AggregatedRange` query that always returns the raw, unaltered Chunks that contain the results, as opposed to sliced-down Chunks that only contain the rows we're interested in.
I.e. `AggregatedRange` can be implemented in terms of `UnslicedAggregatedRange` by slicing down the resulting Chunks.

> [!NOTE]
> `UnslicedAggregatedRange` always shards the data according to the returned primary chunks. This is what makes caching possible in the first place.

Consider e.g. `AggregatedRange(in: #5..=#25, comps: [Position, Radius])` vs. `AggregatedRange(in: #5..=#25, comps: [Position, Radius])`.

`AggregatedRange(in: #5..=#25, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(AggregatedRange(in: #5..=#25, comps: [Position, Radius]))
    #10      Position(10, 10, 10)    Radius(10.0)
    #20      Position(20, 20, 20)    Radius(10.0)
```

`UnslicedAggregatedRange(in: #5..=#25, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(UnslicedAggregatedRange(in: #5..=#25, comps: [Position, Radius])) == {CP1, CP2, CR1, CR2}
  // Primary: CP1
  // Dependencies: [CR1, CR2]
  CHUNK(RangeZip(PoV: Position, chunks: [CP1, CR1, CR2]))
    #10      Position(10, 10, 10)    Radius(10.0)
    #20      Position(20, 20, 20)    Radius(10.0)

  // Primary: CP2
  // Dependencies: [CR1, CR2]
  CHUNK(RangeZip(PoV: Position, chunks: [CP2, CR1, CR2]))
    #0       Position( 0   0,  0)    Radius(0.0)
    #30      Position(30, 30, 30)    Radius(30.0)
```

> [!NOTE]
> Note that there doesn't exist a query (whether it's an `AggregatedLatestAt` or an `AggregatedRange`) that you could run on the underlying store that would yield different data for any of the indices present in this aggregated Chunk. I.e. we can cache this.

TODO: do we somehow need to bootstrap each primary chunk then?
TODO: how does bootstrapping fit into the dependencies?

TODO: we need a dependency-reporting `RangeZip`.


## Dependency tracking

Consider this example from above:
`UnslicedAggregatedLatestAt(at: #15, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(UnslicedAggregatedLatestAt(at: #15, comps: [Position, Radius])) == RangeZip(PoV: Position, chunks: [CP1, CR1])
  // Primary: CP1
  // Dependencies: [CR1]
  CHUNK(RangeZip(PoV: Position, chunks: [CP1, CR1]))
    #10      Position(10, 10, 10)    Radius(10.0)
    #20      Position(20, 20, 20)    Radius(10.0)
```

Consider this other example from above:
`UnslicedAggregatedRange(in: #5..=#25, comps: [Position, Radius])`:
```rust
  frame_nr   component (pov)         component
  --------   ---------------         ---------

CHUNK(UnslicedAggregatedRange(in: #5..=#25, comps: [Position, Radius])) == {CP1, CP2, CR1, CR2}
  // Primary: CP1
  // Dependencies: [CR1, CR2]
  CHUNK(RangeZip(PoV: Position, chunks: [CP1, CR1, CR2]))
    #10      Position(10, 10, 10)    Radius(10.0)
    #20      Position(20, 20, 20)    Radius(10.0)

  // Primary: CP2
  // Dependencies: [CR1, CR2]
  CHUNK(RangeZip(PoV: Position, chunks: [CP2, CR1, CR2]))
    #0       Position( 0   0,  0)    Radius(0.0)
    #30      Position(30, 30, 30)    Radius(30.0)
```

This query yielded two aggregated Chunks that we can cache:
* `CHUNK(RangeZip(PoV: Position, chunks: [CP1, CR1, CR2]))`
* `CHUNK(RangeZip(PoV: Position, chunks: [CP2, CR1, CR2]))`

Both of these aggregated Chunks have two dependencies (i.e. other Chunks whose data was used as part of the aggregation).
In this specific example, they happen to share the same ones.


## Aggregating non-chunk data

The actual data you aggregate into can be whatever you want it to be, as long as:
* Each aggregate corresponds to one aggregated primary Chunk.
* TODO: Each aggregate can be invalidated 


## Invalidation

There is actually no need for invalidation per-se:
* Half of it is the LRU.
* The other half is handled just-in-time, by running the queries every frame.

TODO: all very similar to the query cache, see my other comment in this thread.


## Bootstrapping shenanigans

TODO: demonstrate a failing case


## Thinking about the future

TODO: what does multi-primaries look like?
TODO: What about dataframe APIs? How would that look like?
TODO: Path to a generalized Chunk compute graph?


---


TODO: how does all of this look like in the table of death?
TODO: is there anything we should be worrying about storage-node wise or whatever?
