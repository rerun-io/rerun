---
title: CORS allows rerun.io subdomains by default
hidden: true
type: misc
---

The gRPC server and proxy now accept browser connections from all `rerun.io` subdomains by default, so the web viewer on `app.rerun.io` can connect to a local `serve_grpc` without `--cors-allow-origin`.
