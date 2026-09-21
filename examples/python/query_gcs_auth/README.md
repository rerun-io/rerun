<!--[metadata]
title = "GCS fetches with your own Google credentials"
tags = ["Rerun Hub", "Google Cloud Storage", "Authentication"]
-->

Query a Rerun Hub dataset whose recordings live in Google Cloud Storage, authenticating the direct chunk fetches with an OAuth2 bearer token from your own Google credentials.
Access to the data is then governed by your IAM permissions on the bucket.

Credentials come from [Application Default Credentials](https://cloud.google.com/docs/authentication/application-default-credentials): `gcloud auth application-default login`, a service-account key in `GOOGLE_APPLICATION_CREDENTIALS`, or the attached service account on GCE/GKE/Cloud Run.

```bash
gcloud auth application-default login
REDAP_TOKEN=… python query_gcs_auth.py --url rerun+https://… --dataset my_dataset
```
