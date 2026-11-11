---
title: C++ SDK
order: 200
description: Setting up the C++ SDK
---

If you're using CMake you can add the SDK to your project using `FetchContent`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```

For more details see [Build & distribution](https://github.com/rerun-io/rerun/blob/latest/rerun_cpp/README.md#build--distribution).
The SDK is also available as the `librerun-sdk` conda package and the `rerun-sdk` vcpkg port.

### Bazel

There is no official Bazel support.
A community-maintained example is available at [kyle-figure/bazel-minimal-rerun](https://github.com/kyle-figure/bazel-minimal-rerun).

You'll additionally need to install the [Viewer](./viewer.md).

## Next steps

[Set up a C++ project](../../getting-started/project-setup/cpp.md), then walk through the [Log and Ingest](../../logging-and-ingestion/tutorial.md) tutorial.

-   [C++ API docs](https://ref.rerun.io/docs/cpp)
