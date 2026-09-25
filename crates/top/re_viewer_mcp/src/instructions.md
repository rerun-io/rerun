This MCP drives a live Rerun viewer: it reads the viewer's accessibility tree and synthesizes real input events. Work in an observe → act → verify loop.

Getting oriented:
- Call `rerun_connect` first (it dials the viewer's gRPC server); every other tool errors until then. A server started for a specific viewer is already connected and says so above.
- If no viewer is running, launch one. If the user tells you to work in the background, or no desktop is available, use `--headless`.
- Every Rerun gRPC endpoint serves gRPC server reflection, so `grpcurl -plaintext <host:port> list` shows which services an address speaks (viewer control, SDK proxy, catalog) before you `rerun_connect`, and `describe` shows a service's methods and message types.
- The tool name tells you which of two families it belongs to. A `rerun_*` tool is high level: it names a viewer action and carries it out in one call. Every other tool (`widget_tree`, `click`, `type_text`, `hover`, `scroll`, …) is low level: it drives the widgets one input event at a time, the way a person would.
- Prefer a `rerun_*` tool whenever one fits, and drop to the low-level tools only for what they do not cover. Clicking through the UI costs several calls and a lot of context to achieve what one `rerun_*` call does, and it breaks whenever the layout moves.
- Start most tasks with `rerun_get_viewer_state` to see what is loaded, then `widget_tree` to find widgets, and/or `screenshot` to see the rendered frame.
- Prefer `rerun_run_command` over clicking through menus.

Targeting widgets:
- Prefer locators — an `id` from `widget_tree`, or `role`/`label_contains` — over a raw `pos`. Locators resolve to the widget's current position and survive layout changes; reach for `pos` only when nothing matches.

Pointing the user at something:
- Whenever the user asks where something is — "where is the time cursor?", "how do I hide this panel?", "which button does X?" — answer with `rerun_highlight_rect`, not with prose about the layout. Find it with `get_widget`, pass its `bounds` straight through as `rect`, and give a short `label`. The user is looking at the screen: showing them beats describing a position they then have to hunt for.
- Follow the highlight with a one-line answer. The outline pulses until the user clicks anywhere, and highlighting again replaces it, so point at one thing at a time.

Acting and verifying:
- After an action that changes the UI, confirm it landed: `widget_tree` for the expected state, `screenshot` to look, or `wait_for` to poll until async or animated UI settles.
- Confirm a load with `rerun_get_viewer_state`, not `screenshot`. It names the recordings, timelines and views that appeared, which is what "did it load?" actually asks; a full-window screenshot costs far more and answers less. Screenshot when the question is about looks — framing, layout, colors.
- Use `batch` to act and observe in one round trip (e.g. `click` then `screenshot`), avoiding an extra turn.
- To move through time, call `rerun_get_viewer_state` for the recordings/timelines and their valid ranges, then `rerun_set_time_cursor`.
- The viewer's log messages (INFO and above) since the previous tool call are appended to every tool result. Read them: a warning or error there usually explains what the user is seeing. `rerun_get_viewer_logs` fetches older messages.
- `rerun_close_recordings` clears recordings away. Iterating on a file you keep regenerating leaves a pile of stale recordings behind, which makes `rerun_get_viewer_state` and the UI hard to read — close them.

Reading the data itself:
- Never guess an entity path or a component name. `rerun_get_recording_schema` names every entity of an open recording, the components logged on each, and their Arrow datatypes, whatever the recording was loaded from. Read it before you write a query, a blueprint, or a sentence describing the data.
- That is the schema, not the values: it says what was logged at some point, not what is there at the current time. These tools drive the UI and do not read values — the viewer hosts a catalog server, so read the real values through the Python API.
- `rerun_get_viewer_state` reports that server as `catalog_url`. Hand it straight to `CatalogClient`; do not hardcode a port, since a viewer may serve on any of them.
- A recording's `store_id` is the string `{kind}:{application_id}:{recording_id}`. The kind runs to the first colon, the application id to the next colon not preceded by a backslash, and the recording id is the rest; a colon inside the application id is escaped as `\:` (and a backslash as `\\`). The application id is the catalog dataset's **id** (not its name) and the recording id is the segment id, so look the dataset up by id:
  `CatalogClient(catalog_url).get_dataset(id=application_id)`, then `.schema().entity_paths()` for the schema and `.segment_store(recording_id)` for the data.
- Only local `.rrd` and `.rbl` files are registered, so a recording streamed from an SDK, opened from an `http(s)` URL, or imported from a directory or another file format is absent from the catalog, and its application id is the plain application id rather than a dataset id. Read those from the source instead (`rerun.chunk.RrdReader(path).store(...).schema()` and friends), fetching a URL to a temporary file first.
- `rerun_close_recordings` only closes recordings in the viewer; registered recordings stay in the catalog and can still be read and reopened afterwards.

Conventions:
- Everything is in logical points, one shared coordinate frame: raw `pos`, `resize` dimensions, the `bounds` from `get_widget`, and a default (`pixels_per_point: 1.0`) `screenshot`. So a node's `bounds` center is exactly where to `click`, and a pixel in the screenshot is a logical point. There is no fixed screen size; use `resize` to set the viewport.
