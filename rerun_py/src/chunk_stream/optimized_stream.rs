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
use re_chunk_optimizer::{
    ColumnSelector, MergeSplitOverride, MergeSplitSettings, OptimizationSettings, OwnChunkRule,
};
use re_chunk_store::OptimizationProfile;
use re_log_encoding::ChunkProvider;

use super::error::ChunkPipelineError;
use super::{ChunkStream, ChunkStreamFactory};
use crate::utils::wait_for_future;

/// Resolve the `_optimized_stream()` kwargs into [`OptimizationSettings`].
///
/// Defaults come from [`OptimizationProfile::OBJECT_STORE`]. `chunk_max_bytes == 0` disables the
/// merge/split optimization entirely; passing a row limit with it is an error, because the row
/// guards only exist as guards on the byte target.
//TODO(ab): part of an experimental API to be stabilized and published.
pub fn build_optimization_settings(
    chunk_max_bytes: Option<u64>,
    chunk_max_rows: Option<u64>,
    chunk_max_rows_if_unsorted: Option<u64>,
    target_timeline: Option<String>,
    own_chunk: Option<Vec<OwnChunkRuleArgs>>,
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

    Ok(OptimizationSettings {
        merge_split,
        target_timeline,
        own_chunk,
    })
}

/// One `rerun.experimental._OwnChunkRule`, as the dict its `_to_internal()` produces.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct OwnChunkRuleArgs {
    component_type: Option<String>,
    component: Option<String>,
    entity_filter: Option<String>,
    merge_split: MergeSplitOverrideArgs,
}

#[derive(FromPyObject)]
enum MergeSplitOverrideArgs {
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

        let entity_filter = self
            .entity_filter
            .map(|rules| {
                let filter = EntityPathFilter::parse_forgiving(&rules);
                // The planner resolves without substitutions, so an unresolved `$var` would
                // silently match nothing; reject it here instead.
                filter
                    .clone()
                    .resolve_strict(&EntityPathSubs::empty())
                    .map_err(|err| PyValueError::new_err(err.to_string()))?;
                Ok::<_, PyErr>(filter)
            })
            .transpose()?;

        let merge_split = match self.merge_split {
            MergeSplitOverrideArgs::Named(name) => match name.as_str() {
                "inherit" => MergeSplitOverride::Inherit,
                "passthrough" => MergeSplitOverride::Passthrough,
                other => {
                    return Err(PyValueError::new_err(format!(
                        "unknown merge_split {other:?}; expected \"inherit\", \"passthrough\", \
                         or a `_MergeSplitSettings`"
                    )));
                }
            },
            MergeSplitOverrideArgs::Settings(settings) => {
                let Some(max_bytes) = std::num::NonZeroU64::new(settings.max_bytes) else {
                    return Err(PyValueError::new_err(
                        "an own-chunk merge/split target needs max_bytes > 0; \
                         use merge_split=\"passthrough\" to disable rechunking",
                    ));
                };
                MergeSplitOverride::MergeSplit(MergeSplitSettings {
                    max_bytes,
                    max_rows: std::num::NonZeroU64::new(settings.max_rows),
                    max_rows_if_unsorted: std::num::NonZeroU64::new(settings.max_rows_if_unsorted),
                })
            }
        };

        Ok(OwnChunkRule {
            entity_filter,
            column,
            merge_split,
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
