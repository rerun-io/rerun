# Bucket CORS for the web Viewer

When connected to a Rerun Hub deployment, the Viewer can receive presigned URLs that point straight at the data bucket instead of at the server.
Native clients (the desktop Viewer, SDKs) can always use these URLs.
The Web Viewer runs in a browser, so the browser's same-origin policy blocks it from reading the bucket directly, unless the bucket opts in through CORS.

Without CORS, the Web Viewer still works: it falls back to reading through Rerun Hub.
Data then always flows through an extra hop, subject to any size limits Rerun Hub imposes (e.g., a maximum stream-item size).

## What to configure

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

## Verifying

A preflight probe needs no AWS credentials:

```sh
curl -i -X OPTIONS "https://<bucket>.s3.<region>.amazonaws.com/any-key" \
  -H "Origin: https://<stack>.cloud.rerun.io" \
  -H "Access-Control-Request-Method: GET" \
  -H "Access-Control-Request-Headers: range,if-match"
```

A `200` that echoes the origin in `Access-Control-Allow-Origin` means direct reads will work.
A `403` with `CORSResponse: CORS is not enabled for this bucket` means the configuration is missing.
