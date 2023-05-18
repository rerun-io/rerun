# Depthai Viewer backend

Features:
- [x] color camera video + left/right camera video
- [x] color stereo depth stream
- [x] 3D point cloud
- [x] IMU values + charts so that users can check if it works properly
- [x] Discovery of available OAK cameras + switching between cameras (only 1 can be used at a time)
- [ ] Settings for cameras - various filters, IR, Laser projector - so that user can quickly check the performance of stereo + cameras
- [x] Dropdown list of few most interesting neural models - yolo, hand detection, ...
  - [x] YOLO
  - [x] Face detection
  - [x] Age gender detection
  - [ ] Human Pose
- [ ] Bandwidth statistics
Extra:
- [ ] recording/replay
- [ ] camera calibration - similar to Lukasz' calibration app
- [ ] detailed information about cameras and firmware upgrade
- [ ] visualization for VIO/SLAM when available - similar to Zedfu app


## Develop

```sh
python3 install_requirements.py
```

```sh
source .venv/bin/activate
```
