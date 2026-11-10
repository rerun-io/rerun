---
title: Memory limits respect cgroup limits on Linux
hidden: true
type: misc
---

### Memory limits respect cgroup limits on Linux

On Linux, the default memory limits and percentage limits such as `--memory-limit 50%` are now a fraction of the process's cgroup memory limit, rather than full machine memory.
This is particularly useful when running inside of a container.
