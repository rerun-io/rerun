---
title: Duplicate and segment-limit registration failures are reported by the registration task
hidden: true
type: breaking
---

### Duplicate and segment-limit registration failures are reported by the registration task

`register()` and `register_prefix()` no longer fail up front when a segment layer already exists (`on_duplicate=OnDuplicateSegmentLayer.ERROR`, the default) or when an asset dataset is at its segment limit.
The call now succeeds and returns a handle; `wait()` raises for the duplicate or overflowing segment, and the other segments in the same call are registered.
This matches how schema conflicts and per-segment limits were already reported.
The asset segment limit now counts segment layers, as Rerun Hub does, so a new layer on an existing asset takes a slot.

#### Migration

No SDK change or upgrade is needed: the protocol is unchanged, and every SDK version already reports a failed registration task from `wait()`.
The servers stay compatible with older clients, which stop seeing the up-front error and see the task failure instead.
Code that caught `AlreadyExistsError` around `register()` should catch the `wait()` error instead:

Python, before:

```python
try:
    dataset.register([uri], on_duplicate=OnDuplicateSegmentLayer.ERROR).wait()
except AlreadyExistsError:
    ...
```

Python, after:

```python
try:
    dataset.register([uri], on_duplicate=OnDuplicateSegmentLayer.ERROR).wait()
except ValueError as err:
    # The message names the segment and layer: "… already exists".
    ...
```

Rust, before:

```rust
match client
    .register_with_dataset(dataset_id, sources, IfDuplicateBehavior::Error)
    .await
{
    Err(err) if err.kind == ApiErrorKind::AlreadyExists => {
        // The segment layer was already registered.
    }
    Err(err) => return Err(err),
    Ok(handle) => {
        handle.wait(timeout).await?;
    }
}
```

Rust, after:

```rust
let handle = client
    .register_with_dataset(dataset_id, sources, IfDuplicateBehavior::Error)
    .await?;
match handle.wait(timeout).await {
    Err(RegistrationError::Failed(err)) if err.kind == ApiErrorKind::InvalidArguments => {
        // The message lists the failed segments; the others are registered.
    }
    Err(err) => return Err(err.into()),
    Ok(segment_ids) => {}
}
```
