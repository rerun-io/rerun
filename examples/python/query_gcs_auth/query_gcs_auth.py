#!/usr/bin/env python3
"""Query a dataset stored in Google Cloud Storage using the caller's own Google credentials."""

from __future__ import annotations

import argparse
import os

import google.auth
import google.auth.transport.requests

import rerun as rr

DESCRIPTION = """
Usage: REDAP_TOKEN=… python query_gcs_auth.py --url rerun+https://… --dataset my_dataset

Credentials come from Application Default Credentials: `gcloud auth application-default login`,
a service-account key in `GOOGLE_APPLICATION_CREDENTIALS`, or the attached service account on
GCE/GKE/Cloud Run.
""".strip()

parser = argparse.ArgumentParser(description=DESCRIPTION, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("--url", required=True, help="Catalog server URL, e.g. rerun+https://….")
parser.add_argument("--dataset", required=True, help="Dataset name.")
parser.add_argument("--index", default=None, help="Index to query. Defaults to the first one in the schema.")
parser.add_argument("--limit", type=int, default=1, help="Number of segments to read (0 = all).")
parser.add_argument("--entity", default=None, help='Entity path filter, e.g. "/camera/left". Defaults to everything.')
args = parser.parse_args()

GCS_READ_SCOPE = "https://www.googleapis.com/auth/devstorage.read_only"

credentials, _project = google.auth.default(scopes=[GCS_READ_SCOPE])
transport = google.auth.transport.requests.Request()


def gcs_token() -> str:
    """Current access token, refreshed when expired."""
    if not credentials.valid:
        credentials.refresh(transport)
    return str(credentials.token)


client = rr.catalog.CatalogClient(
    args.url,
    token=os.environ.get("REDAP_TOKEN"),
    object_store_auth=rr.experimental.BearerTokenObjectStoreAuth(gcs_token),
)
dataset = client.get_dataset(name=args.dataset)

index = args.index
if index is None:
    indexes = dataset.schema().index_columns()
    if not indexes:
        raise SystemExit("dataset has no index columns, pass --index")
    index = indexes[0].name

segment_ids = dataset.segment_ids()
if args.limit:
    segment_ids = segment_ids[: args.limit]

view = dataset.filter_segments(segment_ids)
if args.entity:
    view = view.filter_contents(args.entity)

rows = 0
for batch in view.reader(index=index).execute_stream():
    rows += batch.to_pyarrow().num_rows
print(f"{args.dataset}: read {rows} rows from {len(segment_ids)} segments via {index!r}")
