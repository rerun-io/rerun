---
title: JavaScript API uses camelCase
hidden: true
type: breaking
---

The JavaScript viewer API now uses camelCase names.
Previously released snake_case methods, options, and returned fields remain available as deprecated aliases.
These aliases will be removed in a future release.

Before:

```js
await viewer.set_time({ play_state: "paused" });
const recording = (await viewer.get_state()).active_store_id;
```

After:

```js
await viewer.setTime({ playState: "paused" });
const recording = (await viewer.getState()).activeStoreId;
```
