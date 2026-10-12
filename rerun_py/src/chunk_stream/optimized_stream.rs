//! [`ChunkStreamFactory`] adapter over [`re_chunk_optimizer::optimize`].
//!
//! This backs the private `_optimized_stream()` methods of `LazyStore` and `ChunkStore`.

use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use futures::StreamExt as _;
use futures::stream::BoxStream;
use re_chunk::external::re_log_types::{EntityPathFilter, EntityPathSubs};
use re_chunk::{Chunk, ComponentIdentifier, ComponentType};
use re_chunk_index::ChunkProvider;
use re_chunk_optimizer::{
    ColumnSelector, GopBatchingRule, KeyframePlacement, MergeSplitOverride, MergeSplitSettings,
    OptimizationSettings, OwnChunkRule,
};
use re_chunk_store::OptimizationProfile;

use super::error::ChunkPipelineError;
use super::{ChunkStream, ChunkStreamFactory};
use crate::utils::wait_for_future;

/// Resolve the `_optimized_stream()` kwargs into [`OptimizationSettings`].
///
/// Defaults come from [`OptimizationProfile::OBJECT_STORE`]. `chunk_max_bytes == 0` disables the
/// merge/split optimization; passing a row limit with it is an error, because the row guards only
/// exist as guards on the byte target. Video streams are still re-cut at keyframes under the
/// default GoP rule.
//TODO(ab): part of an experimental API to be stabilized and published.
pub fn build_optimization_settings(
    chunk_max_bytes: Option<u64>,
    chunk_max_rows: Option<u64>,
    chunk_max_rows_if_unsorted: Option<u64>,
    target_timeline: Option<String>,
    own_chunk: Option<Vec<OwnChunkRuleArgs>>,
    gop_batching: Option<Vec<GopBatchingRuleArgs>>,
) -> PyResult<OptimizationSettings> {
    let profile = OptimizationProfile::OBJECT_STORE;

    let target_timeline = target_timeline
        .map(|name| {
            re_types_core::TimelineName::try_new(name)
                .map_err(|err| PyValueError::new_err(err.to_string()))
        })
        .transpose()?;

    let max_bytes = chunk_max_bytes.unwrap_or(profile.chunk_max_bytes);
    let merge_split = if let Some(max_bytes) = std::num::NonZeroU64::new(max_bytes) {
        // The kwargs and the profile follow the legacy convention: `0` disables a row guard.
        Some(MergeSplitSettings {
            max_bytes,
            max_rows: std::num::NonZeroU64::new(chunk_max_rows.unwrap_or(profile.chunk_max_rows)),
            max_rows_if_unsorted: std::num::NonZeroU64::new(
                chunk_max_rows_if_unsorted.unwrap_or(profile.chunk_max_rows_if_unsorted),
            ),
        })
    } else {
        if chunk_max_rows.is_some() || chunk_max_rows_if_unsorted.is_some() {
            return Err(PyValueError::new_err(
                "chunk_max_bytes=0 disables the merge/split optimization; \
                 a row limit has no meaning with it",
            ));
        }
        None
    };

    let own_chunk = if let Some(rules) = own_chunk {
        rules
            .into_iter()
            .map(OwnChunkRuleArgs::into_rule)
            .collect::<PyResult<Vec<_>>>()?
    } else {
        // The reflection set iterates in hash order; sort it so the rule order is stable.
        let mut types: Vec<ComponentType> = re_sdk_types::reflection::own_chunk_components()
            .iter()
            .copied()
            .collect();
        types.sort();
        types
            .into_iter()
            .map(|component_type| OwnChunkRule::new(ColumnSelector::Type(component_type)))
            .collect()
    };

    let gop_batching = match gop_batching {
        Some(rules) => rules
            .into_iter()
            .map(GopBatchingRuleArgs::into_rule)
            .collect::<PyResult<Vec<_>>>()?,
        None => vec![GopBatchingRule::default()],
    };

    Ok(OptimizationSettings {
        merge_split,
        target_timeline,
        own_chunk,
        gop_batching,
    })
}

/// One `rerun.experimental._OwnChunkRule`, as the dict its `_to_internal()` produces.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct OwnChunkRuleArgs {
    component_type: Option<String>,
    component: Option<String>,
    entity_filter: Option<String>,
    merge_split: NamedOrSettingsArgs,
}

/// A named choice, or a `_MergeSplitSettings`: the shape of `merge_split`, `samples`, and
/// `keyframes`.
#[derive(FromPyObject)]
enum NamedOrSettingsArgs {
    Named(String),
    Settings(MergeSplitSettingsArgs),
}

/// A `rerun.experimental._MergeSplitSettings`, as a dict; `0` disables a row guard.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
struct MergeSplitSettingsArgs {
    max_bytes: u64,
    max_rows: u64,
    max_rows_if_unsorted: u64,
}

/// One `rerun.experimental._GopBatchingRule`, as the dict its `_to_internal()` produces.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct GopBatchingRuleArgs {
    entity_filter: Option<String>,
    samples: NamedOrSettingsArgs,
    keyframes: NamedOrSettingsArgs,
}

/// Parse entity path filter rules. The planner resolves without substitutions, so an unresolved
/// `$var` would silently match nothing; it is rejected here instead.
fn parse_entity_filter(rules: Option<String>) -> PyResult<Option<EntityPathFilter>> {
    rules
        .map(|rules| {
            let filter = EntityPathFilter::parse_forgiving(&rules);
            filter
                .clone()
                .resolve_strict(&EntityPathSubs::empty())
                .map_err(|err| PyValueError::new_err(err.to_string()))?;
            Ok::<_, PyErr>(filter)
        })
        .transpose()
}

impl MergeSplitSettingsArgs {
    /// `field` names the kwarg in error messages, and `instead` the named choice to suggest.
    fn into_settings(self, field: &str, instead: &str) -> PyResult<MergeSplitSettings> {
        let Some(max_bytes) = std::num::NonZeroU64::new(self.max_bytes) else {
            return Err(PyValueError::new_err(format!(
                "a {field} merge/split target needs max_bytes > 0; use {instead}"
            )));
        };
        Ok(MergeSplitSettings {
            max_bytes,
            max_rows: std::num::NonZeroU64::new(self.max_rows),
            max_rows_if_unsorted: std::num::NonZeroU64::new(self.max_rows_if_unsorted),
        })
    }
}

impl NamedOrSettingsArgs {
    /// `field` names the kwarg in error messages.
    fn into_override(self, field: &str) -> PyResult<MergeSplitOverride> {
        match self {
            Self::Named(name) => match name.as_str() {
                "inherit" => Ok(MergeSplitOverride::Inherit),
                "passthrough" => Ok(MergeSplitOverride::Passthrough),
                other => Err(PyValueError::new_err(format!(
                    "unknown {field} {other:?}; expected \"inherit\", \"passthrough\", \
                     or a `_MergeSplitSettings`"
                ))),
            },
            Self::Settings(settings) => settings
                .into_settings(
                    field,
                    &format!("{field}=\"passthrough\" to disable rechunking"),
                )
                .map(MergeSplitOverride::MergeSplit),
        }
    }

    fn into_placement(self) -> PyResult<KeyframePlacement> {
        match self {
            Self::Named(name) => match name.as_str() {
                "inline" => Ok(KeyframePlacement::Inline),
                "own_chunk" => Ok(KeyframePlacement::OwnChunk(None)),
                other => Err(PyValueError::new_err(format!(
                    "unknown keyframes {other:?}; expected \"inline\", \"own_chunk\", \
                     or a `_MergeSplitSettings`"
                ))),
            },
            Self::Settings(settings) => settings
                .into_settings(
                    "keyframes",
                    "keyframes=\"own_chunk\" to pack toward the global target",
                )
                .map(|settings| KeyframePlacement::OwnChunk(Some(settings))),
        }
    }
}

impl OwnChunkRuleArgs {
    fn into_rule(self) -> PyResult<OwnChunkRule> {
        let column = match (self.component_type, self.component) {
            (Some(component_type), None) => ColumnSelector::Type(
                ComponentType::try_new(component_type)
                    .map_err(|err| PyValueError::new_err(err.to_string()))?,
            ),
            (None, Some(component)) => ColumnSelector::Column(
                ComponentIdentifier::try_new(component)
                    .map_err(|err| PyValueError::new_err(err.to_string()))?,
            ),
            _ => {
                return Err(PyValueError::new_err(
                    "exactly one of `component_type` and `component` must be set",
                ));
            }
        };

        Ok(OwnChunkRule {
            entity_filter: parse_entity_filter(self.entity_filter)?,
            column,
            merge_split: self.merge_split.into_override("merge_split")?,
        })
    }
}

impl GopBatchingRuleArgs {
    fn into_rule(self) -> PyResult<GopBatchingRule> {
        Ok(GopBatchingRule {
            entity_filter: parse_entity_filter(self.entity_filter)?,
            samples: self.samples.into_override("samples")?,
            keyframes: self.keyframes.into_placement()?,
        })
    }
}

/// Factory for optimized chunk streams: each `create()` plans and executes the optimization
/// afresh from the provider's chunk index.
pub struct OptimizedStreamFactory {
    pub provider: Arc<dyn ChunkProvider>,
    pub settings: OptimizationSettings,
}

impl ChunkStreamFactory for OptimizedStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let source = self.provider.source();
        let chunks = re_chunk_optimizer::optimize(Arc::clone(&self.provider), &self.settings)
            .map_err(|err| ChunkPipelineError::Optimize {
                from: source.clone(),
                reason: err.to_string(),
            })?
            .boxed();
        Ok(Box::new(OptimizedChunkStream { chunks, source }))
    }
}

/// Pull-based [`ChunkStream`] driving the stream returned by [`re_chunk_optimizer::optimize`].
struct OptimizedChunkStream {
    chunks: BoxStream<'static, Result<Arc<Chunk>, re_chunk_optimizer::Error>>,

    /// The provider's source string, for error messages.
    source: String,
}

impl ChunkStream for OptimizedChunkStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        Python::attach(|py| wait_for_future(py, self.chunks.next()))
            .transpose()
            .map_err(|err| ChunkPipelineError::Optimize {
                from: self.source.clone(),
                reason: err.to_string(),
            })
    }
}
