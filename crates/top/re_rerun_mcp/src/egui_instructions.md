The UI tools (`ui_widget_tree`, `ui_get_widget`, `ui_click`, `ui_type_text`, `ui_hover`, `ui_scroll`, `ui_screenshot`, …) drive the widgets of the connected viewer one input event at a time, the way a person would: they read the viewer's accessibility tree and synthesize real input events. Work in an observe → act → verify loop.

- They run over the viewer's connection, so call `viewer_connect` first.
- A `viewer_*` tool is high level: it names a viewer action and carries it out in one call. Prefer one whenever it fits, and `viewer_run_command` over clicking through menus: clicking through the UI costs several calls and a lot of context, and breaks whenever the layout moves.

Targeting widgets:
- Prefer locators — an `id` from `ui_widget_tree`, or `role`/`content_contains` — over a raw `pos`. Locators resolve to the widget's current position and survive layout changes; reach for `pos` only when nothing matches.

Pointing the user at something:
- Whenever the user asks where something is — "where is the time cursor?", "how do I hide this panel?", "which button does X?" — answer with `viewer_highlight_rect`, not with prose about the layout. Find it with `ui_get_widget`, pass its `bounds` straight through as `rect`, and give a short `label`. The user is looking at the screen: showing them beats describing a position they then have to hunt for.
- Follow the highlight with a one-line answer. The outline pulses until the user clicks anywhere, and highlighting again replaces it, so point at one thing at a time.

Acting and verifying:
- After an action that changes the UI, confirm it landed: `ui_widget_tree` for the expected state, `ui_screenshot` to look, or `ui_wait_for` to poll until async or animated UI settles.
- Use `ui_batch` to act and observe in one round trip (e.g. `ui_click` then `ui_screenshot`), avoiding an extra turn.

Conventions:
- Everything is in logical points, one shared coordinate frame: raw `pos`, `ui_resize` dimensions, the `bounds` from `ui_get_widget`, and a default (`pixels_per_point: 1.0`) `ui_screenshot`. So a node's `bounds` center is exactly where to `ui_click`, and a pixel in the screenshot is a logical point. There is no fixed screen size; use `ui_resize` to set the viewport.
