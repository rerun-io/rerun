use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeRobotDatasetVersion {
    V1,
    V2,
    V3,
}

impl LeRobotDatasetVersion {
    pub fn find_version(path: impl AsRef<Path>) -> Option<Self> {
        let path = path.as_ref();

        if is_v3_lerobot_dataset(path) {
            Some(Self::V3)
        } else if is_v2_lerobot_dataset(path) {
            Some(Self::V2)
        } else if is_v1_lerobot_dataset(path) {
            Some(Self::V1)
        } else {
            None
        }
    }
}

/// Check whether the provided path contains a `LeRobot` dataset.
pub fn is_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    is_v1_lerobot_dataset(path.as_ref())
        || is_v2_lerobot_dataset(path.as_ref())
        || is_v3_lerobot_dataset(path.as_ref())
}

/// Check whether the provided path contains a v3 `LeRobot` dataset.
fn is_v3_lerobot_dataset(_path: impl AsRef<Path>) -> bool {
    let path = _path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v3 `LeRobot` datasets have per episode metadata stored under `meta/episodes/`
    has_sub_directories(&["meta", "data"], path) && path.join("meta").join("episodes").is_dir()
}

/// Check whether the provided path contains a v2 `LeRobot` dataset.
fn is_v2_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v2 `LeRobot` datasets store the metadata in a `meta` directory,
    // instead of the `meta_data` directory used in v1 datasets.
    has_sub_directories(&["meta", "data"], path)
}

/// Check whether the provided path contains a v1 `LeRobot` dataset.
fn is_v1_lerobot_dataset(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    if !path.is_dir() {
        return false;
    }

    // v1 `LeRobot` datasets stored the metadata in a `meta_data` directory,
    // instead of the `meta` directory used in v2 datasets.
    has_sub_directories(&["meta_data", "data"], path)
}

fn has_sub_directories(directories: &[&str], path: impl AsRef<Path>) -> bool {
    directories.iter().all(|subdir| {
        let subpath = path.as_ref().join(subdir);

        // check that the sub directory exists and is not empty
        subpath.is_dir()
            && subpath
                .read_dir()
                .is_ok_and(|mut contents| contents.next().is_some())
    })
}
