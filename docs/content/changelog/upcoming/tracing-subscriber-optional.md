---
title: "Log setup functions and `tracing-subscriber` are opt-in with feature `log_setup`"
hidden: true
type: breaking
---

### "Log setup for binaries is opt-in with feature `log_setup`"

`re_log` depended on `tracing-subscriber` unconditionally, and every Rerun crate depends on `re_log`.
Because the workspace pins `tracing-subscriber` at `^0.3.23`, a project pinning an earlier `0.3.x` could not depend on Rerun at all — even when using it purely as a SDK, with no viewer and no subscriber of its own.

`tracing-subscriber` now sits behind `re_log`'s existing `setup` feature.
All source-code usage of `tracing-subscriber` was already gated on this feature.

The `rerun` crate exposes `re_log/setup` as a new `log_setup` feature, off by default.
`log_setup` covers the whole of `re_log`'s application-level logging setup: `setup_logging`, `setup_logging_with_filter`, `add_log_msg_receiver`, `LogMsg`, `Receiver`, `Sender`, `FieldValue`, `PanicOnWarnScope`, and `LevelFilter`.
Libraries should probably leave it off and let whoever owns `main` configure logging.

Binaries that set up logging through the re-exported `re_log` must now opt in.
Without the feature, the call no longer resolves:

```
error[E0425]: cannot find function `setup_logging` in crate `re_log`
note: found an item that was configured out
      the item is gated behind the `setup` feature
```

| Before           | After                                                    |
|------------------|----------------------------------------------------------|
| `rerun = "0.37"` | `rerun = { version = "0.37", features = ["log_setup"] }` |

