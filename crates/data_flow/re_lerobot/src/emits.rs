//! Emits: what each feature column becomes, resolved from metadata and config at stream time.

use std::collections::BTreeMap;

use ahash::HashMap;
use re_chunk::EntityPath;

use crate::config::{LeRobotConfig, VideoMode};
use crate::dataset::{Tasks, VideoSource};
use crate::features::{DType, Feature, FeatureKey, SUPPORTED_CHANNEL_COUNTS};

/// The columns the episode timeline is derived from: [`FRAME_INDEX_COLUMN`] when present
/// (a sequence timeline), otherwise [`TIMESTAMP_COLUMN`] (a duration timeline).
pub const FRAME_INDEX_COLUMN: &str = "frame_index";
pub const TIMESTAMP_COLUMN: &str = "timestamp";

/// Language column broadcast to every row of an episode, as opposed to the per-frame
/// `language_events` column.
pub const LANGUAGE_PERSISTENT_COLUMN: &str = "language_persistent";

/// Columns in the `LeRobot` dataset schema that we do not visualize in the viewer, and thus ignore.
pub const LEROBOT_DATASET_IGNORED_COLUMNS: &[&str] = &[
    "episode_index",
    "index",
    FRAME_INDEX_COLUMN,
    TIMESTAMP_COLUMN,
];

/// Resolve a feature key (or derived path) to the entity path it is emitted under.
pub fn entity_path(prefix: &EntityPath, key: &str) -> EntityPath {
    prefix.join(&EntityPath::parse_forgiving(key))
}

/// One lens-shaped output stream of an episode: which column feeds it, where it lands,
/// and what it becomes.
pub struct TabularEmit {
    /// The feature's column name in the episode's parquet data.
    pub column: String,

    /// Fully resolved output entity path (config prefix included).
    pub entity: EntityPath,

    pub kind: TabularEmitKind,
}

/// What an [`TabularEmit`]'s column becomes, with everything read off the raw [`Feature`] at
/// emit-building time.
pub enum TabularEmitKind {
    /// Numeric feature, retagged as the `Scalars` archetype by a lens, plus a static
    /// `SeriesLines` names chunk when the feature has `names` metadata.
    Scalars {
        names: Vec<String>,

        /// Whether the feature's `shape` holds more than one element, so each row carries
        /// a vector of scalars rather than a single one.
        vector: bool,
    },

    /// `task_index` column, joined against the tasks table at execute time.
    TaskLabels,

    /// `subtask_index` column, joined against the subtasks table at execute time.
    SubtaskLabels,

    /// String feature, emitted as a `TextDocument` under the feature's own key.
    Text,

    /// Per-frame encoded image column (`struct<bytes: binary>`), emitted as an
    /// `EncodedImage` chunk, or an `EncodedDepthImage` chunk for 1-channel features.
    Image { depth: bool },
}

/// A language annotation column, fanned out at execute time to text tracks under
/// `prefix`.
pub struct LanguageEmit {
    /// The annotation column name in the episode's parquet data.
    pub column: String,

    /// The entity path prefix the fanned-out text tracks hang beneath.
    pub prefix: EntityPath,
}

/// A video feature, streamed at execute time from its own container.
pub struct VideoEmit {
    /// Fully resolved output entity path (config prefix included).
    pub entity: EntityPath,

    pub source: VideoSource,
}

/// One episode's emits, grouped by executor: each group is the exact input of one
/// execution path in [`crate::convert`].
#[derive(Default)]
pub struct Emits {
    pub tabular: Vec<TabularEmit>,
    pub language: Vec<LanguageEmit>,
    pub videos: Vec<VideoEmit>,
}

/// Resolve the dataset's features into one episode's emits under `config`.
///
/// This is where the raw [`Feature`] is fully consumed: dtype picks the kind, and video
/// features look up their resolved [`VideoSource`] in the episode's address.
pub fn build_emits(
    features: &BTreeMap<FeatureKey, Feature>,
    videos: &HashMap<FeatureKey, VideoSource>,
    tasks: &Tasks,
    config: &LeRobotConfig,
) -> Emits {
    let mut emits = Emits::default();
    for (key, feature) in features
        .iter()
        .filter(|(key, _)| !LEROBOT_DATASET_IGNORED_COLUMNS.contains(&key.as_str()))
    {
        match feature.dtype {
            DType::Image => match feature.num_channels() {
                Some(num_channels) if SUPPORTED_CHANNEL_COUNTS.contains(&num_channels) => {
                    emits.tabular.push(TabularEmit {
                        column: key.as_str().to_owned(),
                        entity: entity_path(&config.entity_path_prefix, key.as_str()),
                        kind: TabularEmitKind::Image {
                            depth: num_channels == 1,
                        },
                    });
                }
                Some(num_channels) => re_log::warn!(
                    "Unsupported channel count {num_channels} (shape: {:?}) for LeRobot image feature `{key}`; only 1- and 3-channel images are supported",
                    feature.shape
                ),
                None => re_log::warn!("Skipping LeRobot image feature `{key}`: its shape is empty"),
            },
            DType::Video => match config.video {
                VideoMode::Native => {
                    // A video feature absent from the address was skipped at open (with a
                    // warning).
                    if let Some(source) = videos.get(key) {
                        emits.videos.push(VideoEmit {
                            entity: entity_path(&config.entity_path_prefix, key.as_str()),
                            source: source.clone(),
                        });
                    }
                }
                VideoMode::Skip => {}
            },
            DType::Language => emits.language.push(LanguageEmit {
                column: key.as_str().to_owned(),
                prefix: config.entity_path_prefix.clone(),
            }),
            DType::Float32 | DType::Float64 => emits.tabular.push(TabularEmit {
                column: key.as_str().to_owned(),
                entity: entity_path(&config.entity_path_prefix, key.as_str()),
                kind: TabularEmitKind::Scalars {
                    names: feature
                        .names
                        .clone()
                        .map(|names| names.0)
                        .unwrap_or_default(),
                    vector: feature.shape.iter().product::<usize>() > 1,
                },
            }),
            // `task_index`/`subtask_index` always refer to the task/subtask
            // descriptions in the dataset metadata; without a table to join
            // against (dropped at open with a warning), the column emits nothing.
            DType::Int64 if key.as_str() == "task_index" && !tasks.tasks.is_empty() => {
                emits.tabular.push(TabularEmit {
                    column: key.as_str().to_owned(),
                    entity: entity_path(&config.entity_path_prefix, "task"),
                    kind: TabularEmitKind::TaskLabels,
                });
            }
            DType::Int64 if key.as_str() == "subtask_index" && !tasks.subtasks.is_empty() => {
                emits.tabular.push(TabularEmit {
                    column: key.as_str().to_owned(),
                    entity: entity_path(&config.entity_path_prefix, "subtask"),
                    kind: TabularEmitKind::SubtaskLabels,
                });
            }
            DType::String => {
                emits.tabular.push(TabularEmit {
                    column: key.as_str().to_owned(),
                    entity: entity_path(&config.entity_path_prefix, key.as_str()),
                    kind: TabularEmitKind::Text,
                });
            }
            DType::Int64 if key.as_str() == "task_index" || key.as_str() == "subtask_index" => {}
            // TODO(RR-5278): Implement support for Int16, Int64, and Bool dtypes.
            DType::Int16 | DType::Int64 | DType::Bool => {
                re_log::warn!(
                    "Loading LeRobot feature ({key}) of dtype `{:?}` into Rerun is not yet implemented",
                    feature.dtype
                );
            }
            // Already warned (naming the dtype) when the metadata was parsed.
            DType::Unknown => {}
        }
    }
    emits
}

/// Pins which feature dtypes emit what. This is the crate's compatibility matrix: when
/// support for a dtype is added or dropped, a test here must change with it.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::{SubtaskIndex, TaskIndex};
    use crate::features::Names;

    fn feature(dtype: DType) -> Feature {
        Feature {
            dtype,
            shape: vec![1],
            names: None,
            info: None,
        }
    }

    fn features(entries: &[(&str, DType)]) -> BTreeMap<FeatureKey, Feature> {
        entries
            .iter()
            .map(|(key, dtype)| (FeatureKey::from(*key), feature(*dtype)))
            .collect()
    }

    fn emits_for(
        features: &BTreeMap<FeatureKey, Feature>,
        videos: &HashMap<FeatureKey, VideoSource>,
        tasks: &Tasks,
        config: &LeRobotConfig,
    ) -> Emits {
        build_emits(features, videos, tasks, config)
    }

    fn entity_paths(emits: &[TabularEmit]) -> Vec<String> {
        emits.iter().map(|emit| emit.entity.to_string()).collect()
    }

    #[test]
    fn floats_emit_scalars_under_their_key() {
        let features = features(&[
            ("action", DType::Float32),
            ("observation.state", DType::Float64),
        ]);
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert_eq!(
            entity_paths(&emits.tabular),
            ["/action", "/observation.state"]
        );
        assert!(
            emits
                .tabular
                .iter()
                .all(|emit| matches!(emit.kind, TabularEmitKind::Scalars { .. }))
        );
    }

    #[test]
    fn scalar_names_are_carried_into_the_emit() {
        let mut features = features(&[("action", DType::Float32)]);
        features.get_mut("action").unwrap().names =
            Some(Names(vec!["shoulder".to_owned(), "elbow".to_owned()]));

        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert!(
            matches!(&emits.tabular[0].kind, TabularEmitKind::Scalars { names, .. } if names == &["shoulder", "elbow"])
        );
    }

    /// The feature shape decides whether rows carry one scalar or a vector of them.
    #[test]
    fn the_shape_decides_scalar_versus_vector() {
        let mut features = features(&[("reward", DType::Float32), ("action", DType::Float32)]);
        features.get_mut("action").unwrap().shape = vec![7];

        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert!(matches!(
            emits.tabular[0].kind,
            TabularEmitKind::Scalars { vector: true, .. }
        ));
        assert!(matches!(
            emits.tabular[1].kind,
            TabularEmitKind::Scalars { vector: false, .. }
        ));
    }

    #[test]
    fn image_features_emit_by_channel_count() {
        for (shape, expected_depth) in [(vec![4, 4, 3], false), (vec![4, 4, 1], true)] {
            let mut features = features(&[("observation.image", DType::Image)]);
            features.get_mut("observation.image").unwrap().shape = shape;

            let emits = emits_for(
                &features,
                &HashMap::default(),
                &Tasks::default(),
                &LeRobotConfig::default(),
            );
            assert_eq!(entity_paths(&emits.tabular), ["/observation.image"]);
            assert!(
                matches!(emits.tabular[0].kind, TabularEmitKind::Image { depth } if depth == expected_depth)
            );
        }

        // Any other channel count emits nothing.
        let mut features = features(&[("observation.image", DType::Image)]);
        features.get_mut("observation.image").unwrap().shape = vec![4, 4, 4];
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );
        assert!(emits.tabular.is_empty());
    }

    #[test]
    fn video_emits_only_with_a_resolved_source() {
        let features = features(&[
            ("observation.image", DType::Video),
            ("observation.wrist", DType::Video),
        ]);
        // Only one of the two video features resolved to a source at open.
        let videos: HashMap<FeatureKey, VideoSource> = std::iter::once((
            FeatureKey::from("observation.image"),
            VideoSource::Asset {
                file: std::path::PathBuf::from("episode_000000.mp4"),
            },
        ))
        .collect();

        let emits = emits_for(
            &features,
            &videos,
            &Tasks::default(),
            &LeRobotConfig::default(),
        );
        assert_eq!(emits.videos.len(), 1);
        assert_eq!(emits.videos[0].entity.to_string(), "/observation.image");
        assert!(matches!(emits.videos[0].source, VideoSource::Asset { .. }));

        let config = LeRobotConfig {
            video: VideoMode::Skip,
            ..Default::default()
        };
        let emits = emits_for(&features, &videos, &Tasks::default(), &config);
        assert!(emits.videos.is_empty());
    }

    #[test]
    fn task_and_subtask_columns_emit_only_with_a_table_to_join() {
        let features = features(&[
            ("task_index", DType::Int64),
            ("subtask_index", DType::Int64),
        ]);

        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );
        assert!(emits.tabular.is_empty());

        let tasks = Tasks {
            tasks: std::iter::once((TaskIndex(0), "pick apple".to_owned())).collect(),
            subtasks: std::iter::once((SubtaskIndex(0), "reach".to_owned())).collect(),
        };
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &tasks,
            &LeRobotConfig::default(),
        );
        assert_eq!(entity_paths(&emits.tabular), ["/subtask", "/task"]);
        assert!(matches!(emits.tabular[1].kind, TabularEmitKind::TaskLabels));
        assert!(matches!(
            emits.tabular[0].kind,
            TabularEmitKind::SubtaskLabels
        ));
    }

    #[test]
    fn language_emits_at_the_prefix() {
        let features = features(&[("annotation.language", DType::Language)]);
        let config = LeRobotConfig {
            entity_path_prefix: EntityPath::from("/robot"),
            ..Default::default()
        };
        let emits = emits_for(&features, &HashMap::default(), &Tasks::default(), &config);

        assert_eq!(emits.language.len(), 1);
        assert_eq!(emits.language[0].prefix.to_string(), "/robot");
        assert_eq!(emits.language[0].column, "annotation.language");
    }

    #[test]
    fn string_features_emit_text_under_their_key() {
        let features = features(&[("subtask", DType::String)]);
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert_eq!(entity_paths(&emits.tabular), ["/subtask"]);
        assert!(matches!(emits.tabular[0].kind, TabularEmitKind::Text));
    }

    #[test]
    fn unsupported_dtypes_emit_nothing() {
        let features = features(&[
            ("flag", DType::Bool),
            ("count", DType::Int64),
            ("small", DType::Int16),
            ("mystery", DType::Unknown),
        ]);
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert!(emits.tabular.is_empty());
        assert!(emits.language.is_empty());
        assert!(emits.videos.is_empty());
    }

    #[test]
    fn bookkeeping_columns_emit_nothing() {
        let features = features(&[
            ("episode_index", DType::Int64),
            ("frame_index", DType::Int64),
            ("index", DType::Int64),
            ("timestamp", DType::Float32),
        ]);
        let emits = emits_for(
            &features,
            &HashMap::default(),
            &Tasks::default(),
            &LeRobotConfig::default(),
        );

        assert!(emits.tabular.is_empty());
        assert!(emits.language.is_empty());
        assert!(emits.videos.is_empty());
    }

    #[test]
    fn the_prefix_lands_before_every_entity() {
        let features = features(&[("action", DType::Float32), ("task_index", DType::Int64)]);
        let tasks = Tasks {
            tasks: std::iter::once((TaskIndex(0), "pick apple".to_owned())).collect(),
            subtasks: HashMap::default(),
        };
        let config = LeRobotConfig {
            entity_path_prefix: EntityPath::from("/robot"),
            ..Default::default()
        };
        let emits = emits_for(&features, &HashMap::default(), &tasks, &config);

        assert_eq!(
            entity_paths(&emits.tabular),
            ["/robot/action", "/robot/task"]
        );
    }
}
