use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::error::LeRobotError;
use crate::{EpisodeIndex, FeatureKey};

#[derive(Default)]
pub struct LeRobotDiagnostics {
    diagnostics: Vec<LeRobotDiagnostic>,
}

pub enum LeRobotDiagnostic {
    SkippedDataFile {
        path: PathBuf,
        err: LeRobotError,
    },
    SkippedFeature {
        feature: FeatureKey,
        err: LeRobotError,
    },
    MetadataWarning {
        reason: String,
    },
    SkippedMetadataRow {
        episode_index: i64,
    },
    FailedFeature {
        episode: EpisodeIndex,
        feature: Option<FeatureKey>,
        err: LeRobotError,
    },
    SkippedEpisode {
        episode: EpisodeIndex,
        err: LeRobotError,
    },
}

#[derive(Default)]
struct DiagnosticSummary {
    count: usize,
    details: Vec<String>,
}

impl DiagnosticSummary {
    fn add(&mut self, detail: &str) {
        self.count += 1;
        self.details.push(detail.to_owned());
    }

    fn message(&self, title: &str) -> Option<String> {
        (self.count > 0).then(|| {
            let details = format_details(self.details.iter().cloned());
            format!("{title} ({}x){details}", self.count)
        })
    }
}

fn format_details(details: impl Iterator<Item = String>) -> String {
    let mut text = String::new();
    for detail in details {
        write!(text, "\n- {detail}").expect("writing to a String cannot fail");
    }
    text
}

struct FileFailure<'a> {
    error: &'a LeRobotError,
    occurrences: usize,
    episodes: BTreeSet<EpisodeIndex>,
}

#[derive(Default)]
struct FeatureFailureSummary<'a> {
    files: BTreeMap<(PathBuf, ErrorKind), FileFailure<'a>>,
    other: DiagnosticSummary,
}

impl<'a> FeatureFailureSummary<'a> {
    fn add(&mut self, episode: EpisodeIndex, feature: Option<&FeatureKey>, err: &'a LeRobotError) {
        let file_error = match err {
            LeRobotError::IO { source, path }
            | LeRobotError::Video {
                source: re_mp4_reader::Mp4Error::Io(source),
                path,
            } => Some((path, source.kind())),
            _ => None,
        };
        if let Some((path, kind)) = file_error {
            let failure = self
                .files
                .entry((path.clone(), kind))
                .or_insert_with(|| FileFailure {
                    error: err,
                    occurrences: 0,
                    episodes: BTreeSet::new(),
                });
            failure.occurrences += 1;
            failure.episodes.insert(episode);
        } else {
            let detail = match feature {
                Some(feature) => format!("episode {}, feature `{feature}`: {err}", episode.0),
                None => format!("episode {}: {err}", episode.0),
            };
            self.other.add(&detail);
        }
    }

    fn message(&self) -> Option<String> {
        let total_groups = self.files.len() + self.other.count;
        if total_groups == 0 {
            return None;
        }
        let title = if self.files.is_empty() {
            format!(
                "Failed to load LeRobot features ({} failure occurrences)",
                self.other.count
            )
        } else {
            let paths: BTreeSet<_> = self.files.keys().map(|(path, _)| path).collect();
            let episodes: BTreeSet<_> = self
                .files
                .values()
                .flat_map(|failure| failure.episodes.iter())
                .collect();
            let mut title = format!(
                "Failed to load LeRobot features ({} files, {} affected episodes",
                paths.len(),
                episodes.len()
            );
            if self.other.count > 0 {
                write!(title, "; {} other failure occurrences", self.other.count)
                    .expect("writing to a String cannot fail");
            }
            title.push(')');
            title
        };
        let details = self.files.values().map(|failure| {
            format!(
                "{} failure occurrences; affected episodes: {}\n{}",
                failure.occurrences,
                format_episodes(&failure.episodes),
                failure.error
            )
        });
        let details = std::iter::chain(details, self.other.details.iter().cloned());
        Some(format!("{title}{}", format_details(details)))
    }
}

fn format_episodes(episodes: &BTreeSet<EpisodeIndex>) -> String {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for episode in episodes {
        if let Some((_, end)) = ranges.last_mut()
            && end.checked_add(1) == Some(episode.0)
        {
            *end = episode.0;
        } else {
            ranges.push((episode.0, episode.0));
        }
    }
    ranges
        .iter()
        .map(|&(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}–{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Default)]
struct DiagnosticSummaries<'a> {
    skipped_episodes: DiagnosticSummary,
    skipped_data_files: DiagnosticSummary,
    skipped_features: DiagnosticSummary,
    failed_features: FeatureFailureSummary<'a>,
    skipped_metadata_rows: DiagnosticSummary,
    metadata_warnings: DiagnosticSummary,
}

impl DiagnosticSummaries<'_> {
    fn messages(&self) -> Vec<String> {
        [
            self.skipped_episodes.message("Skipping episodes"),
            self.skipped_data_files
                .message("Skipping unusable data files and their episodes"),
            self.skipped_features.message("Skipping dataset features"),
            self.failed_features.message(),
            self.skipped_metadata_rows
                .message("Skipping invalid episode metadata rows"),
            self.metadata_warnings.message("LeRobot metadata warnings"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl LeRobotDiagnostics {
    pub fn add(&mut self, diagnostic: LeRobotDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn entries(&self) -> &[LeRobotDiagnostic] {
        &self.diagnostics
    }

    pub fn log_summaries(&self) {
        for message in self.summary_messages() {
            re_log::warn!("{message}");
        }
    }

    /// The collected diagnostics grouped into one message per category, in a fixed
    /// category order. These are the messages [`Self::log_summaries`] emits.
    pub fn summary_messages(&self) -> Vec<String> {
        let mut summaries = DiagnosticSummaries::default();

        for diagnostic in &self.diagnostics {
            let (summary, detail) = match diagnostic {
                LeRobotDiagnostic::SkippedEpisode { episode, err } => (
                    &mut summaries.skipped_episodes,
                    format!("episode {}: {err}", episode.0),
                ),
                LeRobotDiagnostic::SkippedDataFile { err, .. } => {
                    (&mut summaries.skipped_data_files, err.to_string())
                }
                LeRobotDiagnostic::SkippedFeature { feature, err } => (
                    &mut summaries.skipped_features,
                    format!("feature `{feature}`: {err}"),
                ),
                LeRobotDiagnostic::FailedFeature {
                    episode,
                    feature,
                    err,
                } => {
                    summaries
                        .failed_features
                        .add(*episode, feature.as_ref(), err);
                    continue;
                }
                LeRobotDiagnostic::SkippedMetadataRow { episode_index } => (
                    &mut summaries.skipped_metadata_rows,
                    format!(
                        "negative episode, chunk, or file index \
                           (episode_index: {episode_index})"
                    ),
                ),
                LeRobotDiagnostic::MetadataWarning { reason } => {
                    (&mut summaries.metadata_warnings, reason.clone())
                }
            };

            summary.add(&detail);
        }
        summaries.messages()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_groups_preserve_cause_path_and_unique_episodes() {
        let missing = LeRobotError::io(ErrorKind::NotFound.into(), "left/video.mp4");
        let video_missing = LeRobotError::Video {
            source: re_mp4_reader::Mp4Error::Io(ErrorKind::NotFound.into()),
            path: "left/video.mp4".into(),
        };
        let denied = LeRobotError::io(ErrorKind::PermissionDenied.into(), "left/video.mp4");
        let other_camera = LeRobotError::io(ErrorKind::NotFound.into(), "right/video.mp4");
        let mut summary = FeatureFailureSummary::default();
        assert!(summary.message().is_none());
        summary.add(EpisodeIndex(1), None, &missing);
        summary.add(EpisodeIndex(1), None, &missing);
        summary.add(EpisodeIndex(2), None, &video_missing);
        summary.add(EpisodeIndex(2), None, &denied);
        summary.add(EpisodeIndex(2), None, &other_camera);
        assert_eq!(summary.files.len(), 3);
        let group = &summary.files[&("left/video.mp4".into(), ErrorKind::NotFound)];
        assert_eq!(group.occurrences, 3);
        assert_eq!(group.episodes.len(), 2);
        let message = summary.message().unwrap();
        assert!(message.contains("2 files, 2 affected episodes"));
        assert!(message.contains("affected episodes: 1–2"));
    }

    #[test]
    fn non_io_failures_stay_separate_and_all_details_are_shown() {
        let errors: Vec<_> = (0..7)
            .map(|i| LeRobotError::io(ErrorKind::NotFound.into(), format!("{i}.mp4")))
            .collect();
        let decode = LeRobotError::Video {
            source: re_mp4_reader::Mp4Error::NoTimescale,
            path: "0.mp4".into(),
        };
        let mut summary = FeatureFailureSummary::default();
        for err in &errors {
            summary.add(EpisodeIndex(1), None, err);
        }
        summary.add(EpisodeIndex(1), None, &decode);
        summary.add(EpisodeIndex(2), None, &decode);
        assert_eq!(summary.other.count, 2);
        let message = summary.message().unwrap();
        assert!(message.contains("7 files, 1 affected episodes; 2 other failure occurrences"));
        assert_eq!(message.matches("File path:").count(), 9);
        assert!(message.contains("File path: 6.mp4"));
        assert!(!message.contains("additional groups"));
        let mut other = DiagnosticSummary::default();
        for _ in 0..20 {
            other.add("error");
        }
        assert_eq!(other.count, 20);
        assert_eq!(other.details.len(), 20);
    }

    #[test]
    fn episode_ranges_are_sorted_compacted_and_complete() {
        let episodes = [20, 0, 1, 2, 4, 6, 8, 10, 12, 13]
            .into_iter()
            .map(EpisodeIndex)
            .collect();
        assert_eq!(format_episodes(&episodes), "0–2, 4, 6, 8, 10, 12–13, 20");
    }

    #[test]
    fn summaries_separate_episode_and_feature_failures() {
        let mut diagnostics = LeRobotDiagnostics::default();
        assert!(diagnostics.summary_messages().is_empty());
        diagnostics.add(LeRobotDiagnostic::SkippedEpisode {
            episode: EpisodeIndex(1),
            err: LeRobotError::InvalidDatasetInfo("invalid row range".into()),
        });
        for episode in [2, 3] {
            diagnostics.add(LeRobotDiagnostic::FailedFeature {
                episode: EpisodeIndex(episode),
                feature: None,
                err: LeRobotError::io(
                    std::io::Error::from(std::io::ErrorKind::NotFound),
                    "videos/file-001.mp4",
                ),
            });
        }
        let warnings = diagnostics.summary_messages();
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].starts_with("Skipping episodes (1x)\n- episode 1:"));
        assert!(!warnings[0].contains("episode 2:"));
        assert!(
            warnings[1]
                .starts_with("Failed to load LeRobot features (1 files, 2 affected episodes)")
        );
        assert!(warnings[1].contains("affected episodes: 2–3"));
        assert!(warnings[1].contains("File path: videos/file-001.mp4"));
    }
}
