---
title: More ways to read and drive the viewer from Python
hidden: true
type: feature
---

### More ways to read and drive the viewer from Python

`rerun.experimental.ViewerClient` has new methods that read recording, dataset, and table schemas, read and write blueprints, inspect the transform graph, list and run command palette commands, highlight a part of the screen, capture a profile, and take a screenshot as PNG bytes with `take_screenshot`.

Some methods are renamed:
`open_url` is now `open`, which also takes several URLs and returns the store ids it opened;
`close_recordings` is now `close`, which also closes the recordings loaded from a URL;
and `viewer_state` and `viewer_logs` are now `get_state` and `get_logs`.
The old names still work, but are deprecated.
`close`, which shut down a spawned viewer, is now `quit`.

The API is experimental.
