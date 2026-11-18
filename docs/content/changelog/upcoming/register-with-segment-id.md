---
title: Register a recording under any segment ID
hidden: true
type: highlight
---

### Register a recording under any segment ID

`DatasetEntry.register()` takes an optional `segment_id` list, one per URI, to register each recording as a segment other than its recording ID.
It requires a server running Rerun 0.44 or Rerun Hub 0.21 or later.
**IMPORTANT**: once a dataset has such segment, it can no longer be queried by SDKs and viewers older than 0.44.

See [How can I register a recording under another segment ID?](../query-and-transform/catalog/layers.md#how-can-i-register-a-recording-under-another-segment-id) for an example.
