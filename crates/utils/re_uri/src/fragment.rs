use re_log_types::DataPath;

/// We use the `#fragment` of the URI to point to a specific entity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Fragment {
    pub data_path: Option<DataPath>,
}

impl std::fmt::Display for Fragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { data_path } = self;

        if let Some(data_path) = data_path {
            write!(f, "{data_path}")?;
        }

        Ok(())
    }
}

impl Fragment {
    /// Parse fragment, excluding hash
    pub fn parse_forgiving(fragment: &str) -> Self {
        let mut data_path = None;

        match fragment.parse::<DataPath>() {
            Ok(path) => {
                data_path = Some(path);
            }
            Err(err) => {
                re_log::warn_once!(
                    "Failed to parse URL fragment '#{fragment}`: {err} (expected a data path)"
                );
            }
        }

        Self { data_path }
    }
}
