//! A crate for loading and working with `LeRobot` datasets.
//!
//! This module provides functionality to identify and parse `LeRobot` datasets,
//! which consist of metadata and episode data stored in a structured format.
//!
//! # Important
//!
//! This module supports v2 and v3 `LeRobot` datasets!
//!
//! Usage is [`LeRobotDataset::open`] → [`LeRobotDataset::episodes`] → one
//! [`LeRobotDataset::stream`] per episode.
mod config;
mod convert;
mod dataset;
mod emits;
mod error;
mod features;
mod language;
mod lenses;
mod parse_v2;
mod parse_v3;
mod version;

pub use self::config::{LeRobotConfig, VideoMode};
pub use self::dataset::{EpisodeIndex, LeRobotDataset};
pub use self::error::LeRobotError;
pub use self::features::{DType, FeatureKey};
pub use self::version::{LeRobotDatasetVersion, is_lerobot_dataset};
