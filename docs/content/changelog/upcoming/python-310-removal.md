---
title: Python 3.11 is the minimum supported version
hidden: true
type: breaking
---

### Python 3.11 is the minimum supported version

Python 3.10 reaches [end-of-life in October 2026](https://devguide.python.org/versions/).
The Rerun SDK now requires Python 3.11 or later, and importing `rerun` on Python 3.10 raises a `RuntimeError`.

To keep using Rerun, upgrade to Python 3.11 or later.
See [what's new in Python 3.11](https://docs.python.org/3/whatsnew/3.11.html) for what an upgrade involves.
If you must stay on Python 3.10, pin `rerun-sdk<0.39`.

The [supported Python versions table](https://ref.rerun.io/docs/python/main/common#supported-python-versions) lists which Rerun release works with which Python version.
