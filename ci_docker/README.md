If you see this error while trying to publish:
> ERROR: failed to solve: failed to compute cache key: failed to rename: rename /var/lib/containerd/io.containerd.snapshotter.v1.overlayfs/snapshots/new-3352706846 /var/lib/containerd/io.containerd.snapshotter.v1.overlayfs/snapshots/33: file exists: unknown

It means your docker cache is corrupt. Fix it by pruning the cache:
```sh
docker system prune -af
```
