---
title: Why is my blueprint ignored?
order: 400
description: Troubleshoot a sent or loaded recording blueprint
---

Check these common causes:

-   **Different application ID.** For directly streamed or imported recordings, a recording blueprint applies to recordings with the same [application ID](../../log/data-model/recordings.md#application-ids).
    `rr.send_blueprint` uses the application ID of the recording stream it is sent on, so send it on the same stream as your data.
-   **`make_active=False`.** `rr.send_blueprint(…, make_active=False, make_default=True)` only sets the default recording blueprint.
    It becomes active only if no other recording blueprint is active, or when the user resets the recording blueprint.
    The same is true for the `default_blueprint` parameter of `rr.spawn`, `rr.connect_grpc`, `rr.save`, and `rr.serve_grpc`.
-   **The Viewer keeps your edits.** The native Viewer remembers the recording blueprint of each application ID, including changes you made in the UI, across restarts.
    A default recording blueprint does not replace it.
    Use "Reset to default blueprint" from the View menu or command palette, or send the recording blueprint with `make_active=True` (the default for `rr.send_blueprint`).
-   **Wrong override paths.** The keys of `overrides={…}` must be full entity paths, starting at the root.
    `$origin` and wildcards like `/**` are not supported there.
-   **Wrong `origin` or `contents`.** If a view is empty, check that the [entity path filter](../view-concepts/entity-path-filter.md) matches the entity paths you log to.
    Select the view in the Viewer to see its query.


Did the Viewer create views you did not ask for? See [Why the Viewer created these views](recording-layout.md#why-the-viewer-created-these-views).
