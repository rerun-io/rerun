---
title: Python 3.10 is deprecated
hidden: true
type: breaking
---

### Python 3.10 is deprecated

Python 3.10 reaches [end-of-life in October 2026](https://devguide.python.org/versions/).
Importing `rerun` on Python 3.10 now emits a `DeprecationWarning`.
Rerun 0.39 will drop support for it and move the minimum supported version to Python 3.11.

To silence the warning, upgrade to Python 3.11 or later.
See [what's new in Python 3.11](https://docs.python.org/3/whatsnew/3.11.html) for what an upgrade involves.

The [supported Python versions table](https://ref.rerun.io/docs/python/main/common#supported-python-versions) lists which Rerun release works with which Python version.
