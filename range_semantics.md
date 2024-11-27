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

---

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
