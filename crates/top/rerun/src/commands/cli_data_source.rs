use std::path::PathBuf;

use re_data_source::LogDataSource;

pub fn local_recordings_for_assets(
    url_or_paths: &[String],
    assets: &[PathBuf],
) -> anyhow::Result<Vec<PathBuf>> {
    if assets.is_empty() {
        return Ok(Vec::new());
    }

    let recordings = url_or_paths
        .iter()
        .filter(|url_or_path| !url_or_path.starts_with("file://"))
        .filter_map(|url_or_path| {
            if let Some(LogDataSource::File { path, .. }) = LogDataSource::from_uri(
                re_log_types::FileSource::Cli,
                url_or_path,
                &Default::default(),
            ) && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("rrd"))
            {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !recordings.is_empty(),
        "`--asset` needs at least one local `.rrd` recording on the command line"
    );
    Ok(recordings)
}

#[cfg(feature = "native_viewer")]
pub fn take_asset_load_request(
    url_or_path: &str,
    recordings: &mut std::collections::HashSet<PathBuf>,
    assets: &[PathBuf],
) -> Option<re_viewer::external::re_viewer_context::SystemCommand> {
    let path = std::path::absolute(url_or_path).ok()?;
    recordings.remove(&path).then(|| {
        re_viewer::external::re_viewer_context::SystemCommand::LoadDataSource(LogDataSource::File {
            file_source: re_log_types::FileSource::Cli,
            path,
            assets: assets.to_vec(),
        })
    })
}

#[cfg(all(test, feature = "native_viewer"))]
mod tests {
    use re_viewer::external::re_viewer_context::SystemCommand;

    use super::*;

    /// Eligible recordings share the same asset list and each starts asset registration once.
    /// Other recording paths leave the pending registrations unchanged.
    #[test]
    fn recordings_share_assets_once() {
        let assets = vec![PathBuf::from("mesh.rrd")];
        let mut recordings = ["first.rrd", "second.rrd"]
            .map(|path| std::path::absolute(path).unwrap())
            .into_iter()
            .collect();

        assert!(take_asset_load_request("other.rrd", &mut recordings, &assets).is_none());
        assert!(
            take_asset_load_request("https://example.com/first.rrd", &mut recordings, &assets)
                .is_none()
        );
        for recording in ["first.rrd", "second.rrd"] {
            let request = take_asset_load_request(recording, &mut recordings, &assets).unwrap();
            let SystemCommand::LoadDataSource(LogDataSource::File {
                path,
                assets: recording_assets,
                file_source,
            }) = request
            else {
                panic!("expected a file load request with assets");
            };
            assert_eq!(path, std::path::absolute(recording).unwrap());
            assert_eq!(file_source, re_log_types::FileSource::Cli);
            assert_eq!(recording_assets, assets);
            assert!(take_asset_load_request(recording, &mut recordings, &assets).is_none());
        }
        assert!(recordings.is_empty());
    }
}
