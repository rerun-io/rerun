# Custom Collection Adapter

Especially when dealing with large amounts of data, it can be both slow and inconvenient to convert
your data into the components & datatypes provided by the Rerun SDK in order to log it.

This example demonstrates how to solve this using [`rerun::ComponentAdapter`](https://ref.rerun.io/docs/cpp/latest/structrerun_1_1CollectionAdapter.html?speculative-link) for your own types:
Whenever you have data that is continuous in memory and binary compatible with an existing Rerun component,
you can adapt it to map to the respective Rerun component.
<!-- direct link to log method? speculative-link doesn't seem to work with that https://ref.rerun.io/docs/cpp/latest/classrerun_1_1RecordingStream.html#af7a14a7e2c3029ef1679ff9fd680129d -->
For non-temporary objects that live until [`rerun::RecordingStream::log`](https://ref.rerun.io/docs/cpp/latest/classrerun_1_1RecordingStream.html?speculative-link) returns,
it is typically safe to do so with a zero-copy "borrow".
This means that in those cases [`rerun::Collection`](https://ref.rerun.io/docs/cpp/latest/classrerun_1_1Collection.html?speculative-link) will merely store a pointer and a length to your data.

While collection adapters are primarily used with components, they are also useful for all other usages of
rerun's collection type. E.g. the backing buffer of [`rerun::TensorData`](https://ref.rerun.io/docs/cpp/latest/structrerun_1_1datatypes_1_1TensorBuffer.html?speculative-link)
is also a [`rerun::Collection`](https://ref.rerun.io/docs/cpp/latest/classrerun_1_1Collection.html?speculative-link)
allowing you to ingest large amounts of data without a copy and the convenience custom adapters can provide.


To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_custom_collection_adapter
./examples/cpp/minimal/example_custom_collection_adapter
```

---

* 🚧 TODO(#4257): In the future, adapters will be able to provide simple data transformations like strides to be done as a borrow.
* 🚧 TODO(#3977): We plan to provide adapters for common types from Eigen and OpenCV.
