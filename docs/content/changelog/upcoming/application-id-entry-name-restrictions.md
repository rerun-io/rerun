---
title: "Entry-name restrictions now apply to application IDs"
hidden: true
type: breaking
---

### Entry-name restrictions now apply to application IDs

To unify application IDs and [catalog entry names](../concepts/query-and-transform/catalog-object-model.md#catalog), the `EntryName` restrictions now also apply to [`ApplicationId`](../concepts/logging-and-ingestion/recordings.md#application-ids).
Rerun tries to migrate existing application IDs by replacing unsupported characters and dots with hyphens and adding a short hash suffix.
Long application IDs are truncated and receive the same suffix.

Update application IDs to use at most 180 characters and only ASCII alphanumeric characters, underscores, hyphens, spaces, brackets, and colons.
For example, change `my/application` to `my-application`.
