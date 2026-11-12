---
title: Viewer URLs
order: 400
---

A Viewer URL names a location in the Viewer, also called a route: a recording, a page, or something on a Rerun server.
Opening a Viewer URL goes to that location.

| URL                                                              | Location                                                 |
| ---------------------------------------------------------------- | -------------------------------------------------------- |
| `about:recording?app_id=<APP_ID>&recording_id=<RECORDING_ID>`    | A recording that is open in the Viewer                   |
| `about:table?id=<TABLE_ID>`                                      | A table that only exists in the Viewer                   |
| `about:chunk_store?app_id=<APP_ID>&recording_id=<RECORDING_ID>`  | The chunk store browser of an open recording             |
| `about:settings`                                                 | The settings page                                        |
| `rerun+http://<HOST>:<PORT>`                                     | The catalog of a Rerun server                            |
| `rerun+http://<HOST>:<PORT>/entry/<ENTRY_ID>`                    | A table or dataset on a Rerun server                     |
| `rerun+http://<HOST>:<PORT>/dataset/<DATASET_ID>`                | A dataset on a Rerun server                              |
| `rerun+http://<HOST>:<PORT>/folder/<PATH>`                       | A folder of entries on a Rerun server                    |

Query values are URL-encoded, so a recording with the application id `my app` has a URL like `about:recording?app_id=my+app&recording_id=…`.
Use `rerun+https://` for a server with TLS.

A dataset URL with a `segment_id` names a recording on a server.
See [Generate segment URLs in dataframes](../../query-and-transform/dataframes/segment_url.md) for those, and for the `#when=…` fragment that sets the time cursor.

## In the web Viewer

The web Viewer shows its Viewer URL in the address bar, URL-encoded as the `url` query parameter of the page:

```
https://rerun.io/viewer?url=rerun%2Bhttp%3A%2F%2Flocalhost%3A51234%2Fentry%2F<ENTRY_ID>
```

Opening such a page opens the URL in its `url` parameter, and a page with several `url` parameters opens all of them.
This makes the address bar a link to share, except for `about:recording`, `about:chunk_store`, and `about:table`: they name a recording or table by its id, and only work in a Viewer that already has it open.

## Open a file by URL

The `url` parameter can also be an `http://` or `https://` URL of a file:

```
https://rerun.io/viewer?url=https%3A%2F%2Fexample.com%2Frecording.rrd
```

`.rrd` recordings and `.rbl` blueprints are streamed in as they download.
Other file types that the Viewer can [import](../../logging-and-ingestion/import/importers/overview.md) are downloaded first, then loaded.
Repeat the `url` parameter to open several files at once, for example a recording and its blueprint.

The native Viewer takes the same URLs as arguments: `rerun https://example.com/recording.rrd`.

### Presigned and other URLs with a query string

URL-encode the file URL before you put it in the `url` parameter.
A presigned S3 or GCS URL has its own `?` and `&` characters.
Without encoding, the Viewer page reads each `&…` as one of its own parameters, so they are cut from the file URL and the download fails:

```python
from urllib.parse import quote

presigned = "https://my-bucket.s3.amazonaws.com/recording.rrd?X-Amz-Signature=…&X-Amz-Expires=3600"
viewer_url = "https://rerun.io/viewer?url=" + quote(presigned, safe="")
```

### CORS

The web Viewer downloads the file from your browser, so the file host must allow cross-origin requests from the page that runs the Viewer.
Set `Access-Control-Allow-Origin` on the file host to that origin:
- `https://rerun.io` for `https://rerun.io/viewer`.
- `https://app.rerun.io` if you [embed](../../integrations/embed-web.md) `app.rerun.io` in an `<iframe>`, whatever the origin of the surrounding page.
- The origin of your own page if you [host the Viewer yourself](../../integrations/host-web-viewer.md).
For S3 and GCS, this is the bucket's CORS configuration.
The native Viewer does not need CORS.

This is for files. For a `rerun+http://` connection to a gRPC server, see [Connecting to a gRPC server from another origin](../../integrations/embed-web.md#connecting-to-a-grpc-server-from-another-origin-cors).
