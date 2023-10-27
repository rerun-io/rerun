# Custom Component Adapter

Especially when dealing with large amounts of data, it can be both slow and inconvenient to convert
your data into the components & datatypes provided by the Rerun SDK in order to log it.

This example demonstrates how to solve this using `rerun::ComponentBatchAdapter` for your own types:
Whenever you have data that is continuous in memory and binary compatible with an existing Rerun component,
you can adapt it to map to the respective Rerun component.
For non-temporary objects that live until `rerun::RecordingStream::log` returns,
it is typically safe to do so with a zero-copy "borrow".
This means that in those cases `rerun::ComponentBatch` will merely store a pointer and a length to your data.

🚧 TODO(#3794): Right now only component batches can be adapted. This is most prominently an issue for tensors & images
which are single components that store an `std::vector` internally. In the future we'll generalize the adapter concept allowing you to create tensors & images without an additional copy & allocation.
🚧 TODO(#3794): In the future, adapters will be able to provide simple data transformations like strides to be done as a borrow.
🚧 TODO(#3977): We plan to provide adapters for common types from Eigen and OpenCV.


To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_custom_component_adapter
./examples/cpp/minimal/example_custom_component_adapter
```
