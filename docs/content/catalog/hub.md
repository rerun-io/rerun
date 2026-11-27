---
title: Rerun Hub
order: 125
index: true
description: Rerun Hub, our commercial managed offering for production deployments, and how the docs examples apply to it
---

Rerun Hub is our commercial managed offering for production deployments: a persistent, multi-user catalog that indexes your recordings in place in your own object storage. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->
[Reach out](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ?typeform-source=docs) to get access.

## Run the examples on Hub

Most catalog examples in these docs start the open-source server with `rerun server` or `rr.server.Server`, so that you can run them on a laptop.
Rerun Hub serves the same catalog API, so to run an example against Hub, change two things:

1. Connect to your Hub instead of starting a server.
2. Register recordings from object storage, such as `s3://`, instead of from local paths.

Hub reads recordings directly from your bucket, so the bucket must be set up for it (see [Bucket configuration](hub/bucket-configuration.md)).

```python
import rerun as rr

# Instead of: server = rr.server.Server(datasets={"my_dataset": ["recording.rrd"]})
client = rr.catalog.CatalogClient("rerun://api.<stack>.cloud.rerun.io")

dataset = client.create_dataset("my_dataset")
dataset.register(["s3://bucket/recording.rrd"]).wait()
```

Replace `<stack>` with the name of your Hub, as in `https://<stack>.cloud.rerun.io`.
Everything after that, such as [dataframe queries](../query-and-transform/dataframes/dataframe-queries.md), the [training dataloader](../train/dataloader.md), and [segment URLs](../query-and-transform/dataframes/segment_url.md), works without changes.

## Connect to Hub

Log in once with the CLI.
The credentials are stored on your machine, and the SDK and the Viewer use them automatically:

```sh
rerun auth login
```

For CI and other non-interactive environments, create a token with [`rerun auth generate-token`](../reference/cli.md#rerun-auth-generate-token):

```sh
rerun auth generate-token --server rerun://api.<stack>.cloud.rerun.io --expiration "30 days"
```

Then set it in the `REDAP_TOKEN` environment variable, or pass it as `CatalogClient(url, token=…)`.

The variable is named after redap (**Re**run **Da**ta **P**rotocol), the protocol that the SDK and the Viewer use to talk to a catalog server, either Rerun Hub or `rerun server`.

To browse your catalog in the native Viewer, run:

```sh
rerun rerun://api.<stack>.cloud.rerun.io
```

The Web Viewer for your Hub runs at `https://<stack>.cloud.rerun.io`.

## Compared to the open-source server

|                  | Open-source server                  | Rerun Hub                                 |
| ---------------- | ----------------------------------- | ----------------------------------------- |
| Catalog storage  | In memory, lost on restart          | Persistent                                |
| Registration     | Local `file://` paths only          | Object storage, such as `s3://`           |
| Authentication   | None                                | Login or token (see `rerun auth`)         |
| Deployment       | One process that you run yourself   | Managed service                           |

See [The open-source catalog server](oss-server.md) for the details of the local server.
