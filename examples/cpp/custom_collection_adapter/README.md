# Custom collection adapter

Especially when dealing with large amounts of data, it can be both slow and inconvenient to convert
your data into the components & datatypes provided by the Rerun SDK in order to log it.

This example demonstrates how to solve this using [`rerun::ComponentAdapter`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1CollectionAdapter.html) for your own types:
Whenever you have data that is continuous in memory and binary compatible with an existing Rerun component,
you can adapt it to map to the respective Rerun component.
For non-temporary objects that live until [`rerun::RecordingStream::log`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#af7a14a7e2c3029ef1679ff9fd680129d) returns,
it is typically safe to do so with a zero-copy "borrow".
This means that in those cases [`rerun::Collection`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1Collection.html) will merely store a pointer and a length to your data.

While collection adapters are primarily used with components, they are also useful for all other usages of
Rerun's collection type. E.g. the backing buffer of [`rerun::TensorData`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1datatypes_1_1TensorBuffer.html)
is also a [`rerun::Collection`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1Collection.html)
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
