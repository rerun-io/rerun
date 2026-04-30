//! AST-like definitions of lenses.
//!
//! **Note**: Apart from high-level entry points (like [`Lens`]),
//! we should not leak these elements into the public API. This allows us to
//! evolve the definition of lenses over time, if requirements change.

use re_chunk::{Chunk, ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::{ResolvedEntityPathFilter, TimeType};
use re_sdk_types::ComponentDescriptor;
use vec1::Vec1;

use crate::builder::{DeriveLensBuilder, MutateLensBuilder};
use crate::{DynExpr, Selector};

/// A component output.
///
/// Depending on the context in which this output is used, the result from
/// applying the transform should be a list array (1:1) or a list array of list arrays (1:N).
#[derive(Clone, Debug)]
pub struct ComponentOutput {
    pub component_descr: ComponentDescriptor,
    pub selector: Selector<DynExpr>,
}

/// A time extraction output.
#[derive(Clone, Debug)]
pub struct TimeOutput {
    pub timeline_name: TimelineName,
    pub timeline_type: TimeType,
    pub selector: Selector<DynExpr>,
}

/// Controls the row mapping of a derive lens.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Rows {
    /// Each input row maps to exactly one output row.
    #[default]
    OneToOne,

    /// Each input row can map to multiple output rows (scatter/explode).
    OneToMany,
}

/// A derive lens that outputs to the same entity as the input.
///
/// Keeps the original `RowIds` when using [`Rows::OneToOne`] and no time
/// column outputs. Otherwise produces new `RowIds`.
#[derive(Clone)]
pub struct DeriveSameEntityLens {
    pub input: ComponentIdentifier,
    pub rows: Rows,
    pub output_components: Vec1<ComponentOutput>,
    pub output_timelines: Vec<TimeOutput>,
}

impl DeriveSameEntityLens {
    /// Whether this lens can be merged into the prefix chunk.
    pub fn is_merge_candidate(&self) -> bool {
        self.rows == Rows::OneToOne && self.output_timelines.is_empty()
    }
}

/// A derive lens that outputs to a different entity than the input.
#[derive(Clone)]
pub struct DeriveSeparateEntityLens {
    pub input: ComponentIdentifier,
    pub rows: Rows,
    pub target_entity: EntityPath,
    pub output_components: Vec1<ComponentOutput>,
    pub output_timelines: Vec<TimeOutput>,
}

/// A mutate lens modifies the input component in-place.
#[derive(Clone)]
pub struct MutateLens {
    pub input: ComponentIdentifier,
    pub selector: Selector<DynExpr>,
    pub keep_row_ids: bool,
}

/// The internal representation of a [`Lens`].
#[derive(Clone)]
pub enum LensInner {
    /// Modifies the input component in-place.
    Mutate(MutateLens),

    /// Derives new columns, outputting to the same entity.
    DeriveSameEntity(DeriveSameEntityLens),

    /// Derives new columns, outputting to a different entity.
    DeriveSeparateEntity(DeriveSeparateEntityLens),
}

/// A lens that transforms component data from one form to another.
///
/// Works on component columns within a chunk. Because what goes into a chunk
/// is non-deterministic and dependent on the batcher, no assumptions should be
/// made for values across rows.
#[derive(Clone)]
pub struct Lens {
    // Hides implementation details from the public API.
    pub(crate) inner: LensInner,
}

impl From<LensInner> for Lens {
    fn from(inner: LensInner) -> Self {
        Self { inner }
    }
}

impl Lens {
    /// Returns a new [`DeriveLensBuilder`] for the given input component.
    pub fn derive(input: impl Into<ComponentIdentifier>) -> DeriveLensBuilder {
        DeriveLensBuilder::new(input)
    }

    /// Returns a new [`DeriveLensBuilder`] with 1:N row mapping for the given input component.
    pub fn scatter(input: impl Into<ComponentIdentifier>) -> DeriveLensBuilder {
        DeriveLensBuilder::new(input).scatter_rows()
    }

    /// Returns a new [`MutateLensBuilder`] for the given input component.
    pub fn mutate(
        input: impl Into<ComponentIdentifier>,
        selector: impl Into<Selector<DynExpr>>,
    ) -> MutateLensBuilder {
        MutateLensBuilder::new(input, selector)
    }

    /// The input component this lens operates on.
    pub fn input(&self) -> ComponentIdentifier {
        match &self.inner {
            LensInner::Mutate(m) => m.input,
            LensInner::DeriveSameEntity(d) => d.input,
            LensInner::DeriveSeparateEntity(d) => d.input,
        }
    }
}

/// Controls which components are forwarded when applying lenses.
///
/// This is a global setting across all lenses in a [`Lenses`] collection.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum OutputMode {
    /// Forward all original components alongside any lens-produced outputs.
    ForwardAll,

    /// Forward original components that are not consumed by any lens,
    /// alongside any lens-produced outputs.
    ForwardUnmatched,

    /// Only forward lens-produced outputs, dropping all other components.
    DropUnmatched,
}

/// A collection that holds multiple lenses and applies them to chunks.
///
/// Lenses are pre-categorized at add time so per-chunk execution avoids
/// redundant classification work.
#[derive(Clone)]
pub struct Lenses {
    pub(crate) mutates: Vec<(ResolvedEntityPathFilter, MutateLens)>,
    pub(crate) same_entity_derives: Vec<(ResolvedEntityPathFilter, DeriveSameEntityLens)>,
    pub(crate) separate_entity_derives: Vec<(ResolvedEntityPathFilter, DeriveSeparateEntityLens)>,
    pub(crate) mode: OutputMode,
}

impl Lenses {
    /// Creates a new lens collection with the specified mode.
    pub fn new(mode: OutputMode) -> Self {
        Self {
            mutates: Default::default(),
            same_entity_derives: Default::default(),
            separate_entity_derives: Default::default(),
            mode,
        }
    }

    /// Adds a lens that applies to all entity paths.
    pub fn add_lens(self, lens: Lens) -> Self {
        self.add_lens_with_filter(re_log_types::EntityPathFilter::all(), lens)
    }

    /// Adds a lens with an entity path filter.
    pub fn add_lens_with_filter(
        mut self,
        filter: re_log_types::EntityPathFilter,
        lens: Lens,
    ) -> Self {
        let filter = filter.resolve_without_substitutions();
        match lens.inner {
            LensInner::Mutate(mutate) => {
                self.mutates.push((filter, mutate));
            }
            LensInner::DeriveSameEntity(derive) => {
                self.same_entity_derives.push((filter, derive));
            }
            LensInner::DeriveSeparateEntity(derive) => {
                self.separate_entity_derives.push((filter, derive));
            }
        }
        self
    }

    /// Sets the output mode for this collection.
    pub fn set_output_mode(&mut self, mode: OutputMode) {
        self.mode = mode;
    }

    /// Applies all relevant lenses and returns the results.
    ///
    /// Each lens matches by input component. Collisions on output component
    /// identifiers are detected: the first lens wins and duplicates are skipped
    /// with a warning.
    pub fn apply<'a>(
        &'a self,
        // TODO(grtlr): Let's take ownership here.
        chunk: &'a Chunk,
    ) -> impl Iterator<Item = Result<Chunk, crate::LensError>> + 'a {
        crate::execute::execute(self, chunk)
    }
}
