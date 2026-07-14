---
title: C++ SDK
order: 200
---

If you're using CMake you can add the SDK to your project using `FetchContent`:

```cmake
include(FetchContent)
FetchContent_Declare(rerun_sdk URL
    https://github.com/rerun-io/rerun/releases/latest/download/rerun_cpp_sdk.zip)
FetchContent_MakeAvailable(rerun_sdk)
```

For more details see [Build & Distribution](https://ref.rerun.io/docs/cpp/stable/index.html#autotoc_md8) in the C++ reference documentation.

You'll additionally need to install the [Viewer](./viewer.md).

## Next steps

[Set up a C++ project](../../getting-started/project-setup/cpp.md), then walk through the [Log and Ingest](../../getting-started/data-in.md) tutorial.
