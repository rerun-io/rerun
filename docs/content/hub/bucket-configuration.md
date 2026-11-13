---
title: Bucket configuration
order: 0
description: Configure your data bucket for direct Web Viewer reads and for uploads
---

Rerun Hub reads data directly from your bucket, and clients upload to it.
This page lists the bucket settings these features need.

## CORS

When connected to a Rerun Hub deployment, the Viewer can receive presigned URLs that point straight at the data bucket instead of at the server.
Native clients (the desktop Viewer, SDKs) can always use these URLs.
The Web Viewer runs in a browser, so the browser's same-origin policy blocks it from reading the bucket directly, unless the bucket opts in through CORS.

Without CORS, the Web Viewer still works: it falls back to reading through Rerun Hub.
Data then always flows through an extra hop, subject to any size limits Rerun Hub imposes (e.g., a maximum stream-item size).

### What to configure

The typical setup: the Web Viewer runs at `https://<stack>.cloud.rerun.io`, and the data lives in a customer-owned S3 bucket.
The bucket needs a CORS configuration that allows that origin to read.

What the Viewer sends and reads:

* Ranged `GET`s with a `Range` header, plus `If-Match` when the dataset carries an `ETag`.
  Neither header is CORS-safelisted, so the browser sends an `OPTIONS` preflight before every read (cached for `MaxAgeSeconds`).
* The Viewer validates `Content-Range` and reads `ETag` and `Last-Modified` from responses.
  The bucket must expose these headers, or the Viewer rejects the response and falls back.

`cors.json`:

```json
{
  "CORSRules": [
    {
      "AllowedOrigins": ["https://<customer>.cloud.rerun.io"],
      "AllowedMethods": ["GET", "HEAD"],
      "AllowedHeaders": ["Range", "If-Match"],
      "ExposeHeaders": ["Content-Range", "ETag", "Last-Modified", "Accept-Ranges"],
      "MaxAgeSeconds": 3600
    }
  ]
}
```

```sh
aws s3api put-bucket-cors --bucket <bucket> --cors-configuration file://cors.json
```

S3 allows one `*` wildcard per origin (`https://*.cloud.rerun.io`), but prefer explicit origins.

Other object stores take the same settings in their own format (GCS folds allowed and exposed headers into one `responseHeader` list; Azure Blob Storage configures CORS per storage account).

### Multipart uploads from browsers

For browser clients uploading parts directly to storage, also allow `PUT` and every header required by the presigned requests.
Expose `ETag` (`"ExposeHeaders": ["ETag"]` in S3) so the client can read each part's receipt and submit it when completing the upload.
Without this, a part can upload successfully while the browser hides the receipt needed for completion.
For both single-request and multipart uploads, extend the example above with `"AllowedMethods": ["GET", "HEAD", "PUT"]` and `"AllowedHeaders": ["Range", "If-Match", "If-None-Match"]`.
Keep the existing exposed headers, including `ETag`.
Upload failures do not fall back through Rerun Hub; the client aborts the multipart session when possible.

### Verifying

A preflight probe needs no AWS credentials:

```sh
curl -i -X OPTIONS "https://<bucket>.s3.<region>.amazonaws.com/any-key" \
  -H "Origin: https://<stack>.cloud.rerun.io" \
  -H "Access-Control-Request-Method: GET" \
  -H "Access-Control-Request-Headers: range,if-match"
```

A `200` that echoes the origin in `Access-Control-Allow-Origin` means direct reads will work.
A `403` with `CORSResponse: CORS is not enabled for this bucket` means the configuration is missing.

## Incomplete multipart uploads

Large uploads use multipart uploads.
Clients abort an upload when it fails or is cancelled, so incomplete uploads should remain only after a client or server crashes.
S3 keeps the parts of an incomplete upload, and bills their storage, until the upload is aborted.
Add a lifecycle rule that aborts them, scoped to the upload prefix.

`lifecycle.json`:

```json
{
  "Rules": [
    {
      "ID": "abort-incomplete-rerun-uploads",
      "Status": "Enabled",
      "Filter": { "Prefix": "<upload-prefix>/" },
      "AbortIncompleteMultipartUpload": { "DaysAfterInitiation": 2 }
    }
  ]
}
```

```sh
aws s3api put-bucket-lifecycle-configuration --bucket <bucket> --lifecycle-configuration file://lifecycle.json
```

This command replaces the bucket's whole lifecycle configuration.
If the bucket already has rules, add this rule to them.
Choose `DaysAfterInitiation` longer than the longest upload you expect, so that the rule does not abort an upload in progress.
The rule does not delete completed objects.
