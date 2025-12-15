# Bazel

We are currently undergoing a Bazel-ification of our crates.

Here are some limitations:

* [ ] `pixi run codegen` still is a manual process and lives outside the Bazel ecosystem.
* [ ] Documentation-only examples (`examples/rust/chess_robby_fischer`, `examples/rust/revy`) require minimal `Cargo.toml` files to avoid being incorrectly picked up by `crates_universe`.
* [ ] `BUILD.bazel` files now support features matching `pixi run rerun` (`release_no_web_viewer`):
  * [ ] `re_sdk_types` includes `ecolor`, `egui_plot` (for marker shapes), `glam`, `image`, and `video` features.
  * [ ] `re_sdk` now includes `data_loaders` and `server` features with dependencies `re_data_loader` and `re_log_channel`.
  * [ ] `re_auth` has both `cli` and `oauth` features enabled for CLI authentication commands.
  * [ ] `re_ui` has `analytics` and `testing` features enabled (testing required for test crates).
  * [ ] `re_video` has explicit `rustc_flags = ["--cfg=with_ffmpeg", "--cfg=native"]` for proper cfg gating of FFmpegError visibility.
  * [ ] `re_renderer`, `re_viewer`, and `rerun` use `CARGO_MANIFEST_DIR` rustc_env for `document_features` proc-macro.
  * [ ] All 32 viewer crates now have BUILD.bazel files.
  * [ ] Core type crates (`re_log_types`, `re_types_core`, `re_tuid`, `re_string_interner`, `re_build_info`) have `serde` feature enabled globally since it's commonly needed.
  * [ ] `re_byte_size` has the `glam` feature enabled for `SizeBytes` implementations on glam types.
  * [ ] `re_log_encoding` has the `decoder`, `encoder`, and `stream_from_http` features enabled.

  ## TODO

  * [ ] `build_info` via `stamp` and _workspace status commands_.
  * [ ] Find meaningful set of features -> Look at our CI runs for this!
  * [ ] Ensure that the same amounts of test run (and pass) as cargo
  * [ ] Include `platforms` and enable web assembly builds
