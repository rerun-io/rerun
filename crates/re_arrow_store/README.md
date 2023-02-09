# Rerun Arrow Store
[Apache Arrow](https://arrow.apache.org/) is a language-independent columnar memory format for arbitrary data.

The `re_arrow_store` crate is an in-memory time series database for Rerun log data. It is indexed by Entity path, component, timeline, and time. It supports out-of-order insertions, and fast `O(log(N))` queries.
