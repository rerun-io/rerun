---
title: Dataset schema is fetched once per revision
hidden: true
type: misc
---

`DatasetEntry` and `DatasetSelection` now cache the dataset schema per dataset revision, so repeated `reader()` calls no longer each cost a round trip to the server.
A registration or unregistration made from the same process shows up in the next read.
A change made by another client shows up once this process sees a newer revision of the dataset.
