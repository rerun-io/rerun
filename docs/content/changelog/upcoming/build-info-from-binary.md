---
title: Build info is passed in by the binary
hidden: true
type: breaking
---

`rerun::build_info()` and `re_viewer::build_info()` have been removed.
`re_server::RerunCloudHandlerBuilder::new`, `re_server::Args::create_server_handle`, and `re_server::Args::run_async` now take the `BuildInfo` of the binary hosting the server.

To get the build info of your own binary, call `re_build_tools::export_build_info_vars_for_crate("<your crate>")` from its `build.rs`, and use `re_build_info::build_info!()`:

```rust
// Before
re_crash_handler::install_crash_handlers(re_viewer::build_info());

// After
re_crash_handler::install_crash_handlers(re_build_info::build_info!());
```

If you only need the version of the Rerun crates, use `re_build_info::CrateVersion::LOCAL`.
