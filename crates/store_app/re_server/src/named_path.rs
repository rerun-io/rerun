use std::path::PathBuf;
use std::str::FromStr;

use re_protos::EntryName;

#[derive(Debug, Clone)]
pub struct NamedPath {
    pub name: Option<String>,
    pub path: PathBuf,
}

/// A named collection of paths.
#[derive(Debug, Clone)]
pub struct NamedPathCollection {
    pub name: EntryName,
    pub paths: Vec<PathBuf>,
}

impl FromStr for NamedPath {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((name, path)) = s.split_once('=') {
            Ok(Self {
                name: Some(name.to_owned()),
                path: PathBuf::from(path),
            })
        } else {
            Ok(Self {
                name: None,
                path: PathBuf::from(s),
            })
        }
    }
}
