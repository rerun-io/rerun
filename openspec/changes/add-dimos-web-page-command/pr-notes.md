# PR Notes: DimOS Web Page View command

## Canonical API

The preferred, stable way to create a Web Page View remains the Rerun blueprint API:

```python
import rerun as rr
import rerun.blueprint as rrb

rr.send_blueprint(
    rrb.Blueprint(
        rrb.WebPageView(
            name="Viser",
            config=rrb.WebPageViewConfig(
                url="http://127.0.0.1:8095/",
                show_navigation_controls=True,
            ),
        )
    )
)
```

## DimOS websocket command

The DimOS websocket command is experimental, DimOS-only, and removable. It is a convenience wrapper for environments that already speak to the DimOS viewer websocket and cannot rely on a generated Rerun Python SDK from this branch.

```json
{
  "type": "open_web_page_view",
  "panel_id": "viser",
  "title": "Viser",
  "url": "http://127.0.0.1:8095/",
  "show_navigation_controls": true
}
```

The command translates to normal Web Page View blueprint state. It does not mutate native webview internals directly and does not expose arbitrary viewport layout control.

## File-scope justification

The large generated-file blast radius belongs to the core Web Page View feature. This follow-up command stays intentionally small:

- `dimos/src/interaction/ws.rs` parses inbound commands and preserves existing outbound event JSON.
- `dimos/src/viewer.rs` drains validated commands from the websocket queue.
- `crates/viewer/re_viewer/src/app.rs` and `crates/viewer/re_viewer/src/app_state.rs` expose and implement a tiny public wrapper API that translates a request into existing blueprint/view/focus operations.

No new SDK schema, generated API, or native webview backend behavior is introduced by this command.
