---
title: Viewer URLs
order: 6
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
See [Generate segment URLs in dataframes](../../howto/query-and-transform/segment_url.md) for those, and for the `#when=…` fragment that sets the time cursor.

## In the web Viewer

The web Viewer shows its Viewer URL in the address bar, URL-encoded as the `url` query parameter of the page:

```
https://rerun.io/viewer?url=rerun%2Bhttp%3A%2F%2Flocalhost%3A51234%2Fentry%2F<ENTRY_ID>
```

Opening such a page opens the URL in its `url` parameter, and a page with several `url` parameters opens all of them.
This makes the address bar a link to share, except for `about:recording`, `about:chunk_store`, and `about:table`: they name a recording or table by its id, and only work in a Viewer that already has it open.
